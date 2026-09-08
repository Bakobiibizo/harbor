use crate::{
    error::AppError,
    services::{game_signing_protocol::HarborGameCreatorSignatureEnvelope, CryptoService},
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;

pub const MAX_HARBOR_GAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_INDEX_BYTES: usize = 256 * 1024;
const MAX_FILES: usize = 1_024;
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const HEADER_BYTES: usize = 16;
const MAGIC: &[u8] = b"HARBORGAME";
const EMPTY_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const ED25519_SPKI_PREFIX: &[u8] = &[
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedGamePackage {
    pub archive_digest: String,
    pub byte_length: usize,
    pub creator_peer_id: String,
    pub game_id: String,
    pub package_digest: String,
    pub permissions: Vec<String>,
    pub title: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoreApprovalPayload {
    pub approved_at: i64,
    pub archive_digest: String,
    pub creator_peer_id: String,
    pub domain: String,
    pub game_id: String,
    pub reviewer_id: String,
    pub version: u16,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoreApproval {
    pub algorithm: String,
    pub payload: StoreApprovalPayload,
    pub public_key: String,
    pub signature: String,
}

pub fn verify_game_package(bytes: &[u8]) -> Result<VerifiedGamePackage, AppError> {
    let files = read_archive(bytes)?;
    let manifest = parse_canonical_json(required_file(&files, "runtime.json")?, "runtime.json")?;
    validate_manifest(&manifest)?;
    parse_canonical_json(required_file(&files, "assets.json")?, "assets.json")?;
    let signature: HarborGameCreatorSignatureEnvelope = serde_json::from_value(
        parse_canonical_json(required_file(&files, "package.sig")?, "package.sig")?,
    )
    .map_err(|_| invalid("Creator signature envelope is malformed"))?;
    validate_creator_signature_shape(&signature)?;

    let manifest_object = manifest
        .as_object()
        .ok_or_else(|| invalid("Runtime manifest must be an object"))?;
    let assets = object(manifest_object, "assets")?;
    let mut expected_paths: BTreeSet<String> = [
        "assets.json".to_string(),
        "game.wasm".to_string(),
        "package.sig".to_string(),
        "runtime.json".to_string(),
    ]
    .into_iter()
    .collect();
    let items = assets
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Runtime asset items must be an array"))?;
    for asset in items {
        let asset = asset
            .as_object()
            .ok_or_else(|| invalid("Runtime asset entry must be an object"))?;
        let path = string(asset, "path")?;
        if !expected_paths.insert(path.to_string()) {
            return Err(invalid("Runtime asset paths must be unique"));
        }
    }
    if files.len() != expected_paths.len()
        || files.keys().any(|path| !expected_paths.contains(path))
    {
        return Err(invalid("Archive contains missing or undeclared files"));
    }

    verify_manifest_artifacts(&manifest, &files)?;
    let manifest_hash = compute_manifest_hash(&manifest)?;
    let integrity = object(manifest_object, "integrity")?;
    if string(integrity, "manifestHash")? != manifest_hash {
        return Err(invalid("Runtime manifest digest does not match"));
    }
    let package_digest = compute_content_digest(&files, &manifest)?;
    let signature_metadata = object(integrity, "signature")?;
    if string(signature_metadata, "signedManifestHash")? != package_digest
        || signature.payload.package_digest != package_digest
    {
        return Err(invalid(
            "Creator signature or manifest digest does not match package",
        ));
    }
    if string(signature_metadata, "keyId")? != signature.payload.creator_peer_id {
        return Err(invalid(
            "Runtime manifest signature key does not match creator",
        ));
    }

    let metadata = object(manifest_object, "metadata")?;
    let platform_api = object(manifest_object, "platformApi")?;
    let permissions = string_array(platform_api, "permissions")?;
    let mut sorted_permissions = permissions.clone();
    sorted_permissions.sort();
    if signature.payload.game_id != string(metadata, "gameId")?
        || signature.payload.version_id != string(metadata, "versionId")?
        || signature.payload.title != string(metadata, "title")?
        || signature.payload.permissions != sorted_permissions
    {
        return Err(invalid(
            "Creator signature metadata does not match runtime manifest",
        ));
    }
    verify_creator_signature(&signature)?;

    Ok(VerifiedGamePackage {
        archive_digest: sha256(bytes),
        byte_length: bytes.len(),
        creator_peer_id: signature.payload.creator_peer_id,
        game_id: signature.payload.game_id,
        package_digest,
        permissions,
        title: signature.payload.title,
        version_id: signature.payload.version_id,
    })
}

pub fn verify_store_approval(
    approval: &StoreApproval,
    package: &VerifiedGamePackage,
    trusted_public_key: &str,
) -> Result<(), AppError> {
    if approval.algorithm != "ed25519"
        || approval.payload.domain != "neo-grounds.harbor-store.approval.v1"
        || approval.payload.version != 1
        || approval.public_key != trusted_public_key
        || approval.payload.archive_digest != package.archive_digest
        || approval.payload.creator_peer_id != package.creator_peer_id
        || approval.payload.game_id != package.game_id
        || approval.payload.version_id != package.version_id
    {
        return Err(invalid(
            "Store approval does not match the trusted package metadata",
        ));
    }
    let public_der = hex::decode(&approval.public_key)
        .map_err(|_| invalid("Store approval public key is not hexadecimal"))?;
    if public_der.len() != ED25519_SPKI_PREFIX.len() + 32
        || &public_der[..ED25519_SPKI_PREFIX.len()] != ED25519_SPKI_PREFIX
    {
        return Err(invalid("Store approval public key format is invalid"));
    }
    let public_bytes: [u8; 32] = public_der[ED25519_SPKI_PREFIX.len()..]
        .try_into()
        .map_err(|_| invalid("Store approval public key length is invalid"))?;
    let key = VerifyingKey::from_bytes(&public_bytes)
        .map_err(|_| invalid("Store approval public key is invalid"))?;
    let signature_bytes = hex::decode(&approval.signature)
        .map_err(|_| invalid("Store approval signature is not hexadecimal"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| invalid("Store approval signature length is invalid"))?;
    let payload = serde_json::to_vec(&approval.payload)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    key.verify(&payload, &signature)
        .map_err(|_| invalid("Store approval signature is invalid"))
}

fn read_archive(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, AppError> {
    if !(HEADER_BYTES..=MAX_HARBOR_GAME_BYTES).contains(&bytes.len()) {
        return Err(invalid("Archive size is outside the supported range"));
    }
    if &bytes[..MAGIC.len()] != MAGIC || u16::from_be_bytes([bytes[10], bytes[11]]) != 1 {
        return Err(invalid("Archive header or version is invalid"));
    }
    let index_length = u32::from_be_bytes(bytes[12..16].try_into().expect("fixed header")) as usize;
    if index_length == 0
        || index_length > MAX_INDEX_BYTES
        || HEADER_BYTES + index_length > bytes.len()
    {
        return Err(invalid("Archive index size is invalid"));
    }
    let index_value = parse_canonical_json(
        &bytes[HEADER_BYTES..HEADER_BYTES + index_length],
        "archive index",
    )?;
    let index = index_value
        .as_object()
        .ok_or_else(|| invalid("Archive index must be an object"))?;
    exact_keys(index, &["entries", "format", "version"], "archive index")?;
    if string(index, "format")? != "harborgame-archive" || integer(index, "version")? != 1 {
        return Err(invalid("Archive index format is invalid"));
    }
    let entries = index
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Archive entries must be an array"))?;
    if entries.is_empty() || entries.len() > MAX_FILES {
        return Err(invalid("Archive file count is invalid"));
    }
    let payload_start = HEADER_BYTES + index_length;
    let payload_length = bytes.len() - payload_start;
    let mut files = BTreeMap::new();
    let mut expected_offset = 0usize;
    let mut previous_path: Option<&str> = None;
    for value in entries {
        let entry = value
            .as_object()
            .ok_or_else(|| invalid("Archive entry must be an object"))?;
        exact_keys(
            entry,
            &["byteLength", "offset", "path", "sha256"],
            "archive entry",
        )?;
        let path = string(entry, "path")?;
        validate_path(path)?;
        if previous_path.is_some_and(|previous| previous.as_bytes() >= path.as_bytes()) {
            return Err(invalid("Archive paths are duplicated or out of order"));
        }
        let offset = unsigned(entry, "offset")?;
        let byte_length = unsigned(entry, "byteLength")?;
        if offset != expected_offset || byte_length > MAX_FILE_BYTES {
            return Err(invalid("Archive entry offset or size is invalid"));
        }
        let end = offset
            .checked_add(byte_length)
            .filter(|end| *end <= payload_length)
            .ok_or_else(|| invalid("Archive entry is truncated"))?;
        let file_bytes = &bytes[payload_start + offset..payload_start + end];
        if string(entry, "sha256")? != sha256(file_bytes) {
            return Err(invalid(&format!("Archive digest mismatch: {path}")));
        }
        files.insert(path.to_string(), file_bytes.to_vec());
        expected_offset = end;
        previous_path = Some(path);
    }
    if expected_offset != payload_length {
        return Err(invalid("Archive payload has gaps or trailing bytes"));
    }
    Ok(files)
}

fn create_archive(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, AppError> {
    let mut files = files.to_vec();
    files.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    let mut offset = 0usize;
    let entries: Vec<Value> = files
        .iter()
        .map(|(path, bytes)| {
            let entry = serde_json::json!({
                "byteLength": bytes.len(),
                "offset": offset,
                "path": path,
                "sha256": sha256(bytes),
            });
            offset += bytes.len();
            entry
        })
        .collect();
    let index = serde_json::to_vec(&serde_json::json!({
        "entries": entries,
        "format": "harborgame-archive",
        "version": 1,
    }))
    .map_err(|error| AppError::Serialization(error.to_string()))?;
    let index_length: u32 = index
        .len()
        .try_into()
        .map_err(|_| invalid("Normalized archive index is too large"))?;
    let mut archive = Vec::with_capacity(HEADER_BYTES + index.len() + offset);
    archive.extend_from_slice(MAGIC);
    archive.extend_from_slice(&1u16.to_be_bytes());
    archive.extend_from_slice(&index_length.to_be_bytes());
    archive.extend_from_slice(&index);
    for (_, bytes) in files {
        archive.extend_from_slice(&bytes);
    }
    Ok(archive)
}

fn validate_manifest(manifest: &Value) -> Result<(), AppError> {
    let manifest = manifest
        .as_object()
        .ok_or_else(|| invalid("Runtime manifest must be an object"))?;
    let compatibility = object(manifest, "compatibility")?;
    exact_keys(
        compatibility,
        &[
            "hostIntegration",
            "minimumPlayerVersion",
            "packageFormat",
            "renderer",
            "runtimeAbi",
            "schemaVersion",
        ],
        "runtime compatibility",
    )?;
    if integer(compatibility, "schemaVersion")? != 1
        || string(compatibility, "packageFormat")? != "neo-grounds-runtime-package"
        || string(compatibility, "runtimeAbi")? != "neo-grounds-wasm-component-v1"
        || string(compatibility, "renderer")? != "canvas2d-command-buffer"
        || string(compatibility, "hostIntegration")? != "worker-host-canvas-proxy"
    {
        return Err(invalid("Runtime compatibility is unsupported"));
    }
    let metadata = object(manifest, "metadata")?;
    validate_identifier(string(metadata, "gameId")?, "game ID")?;
    validate_identifier(string(metadata, "versionId")?, "version ID")?;
    validate_text(string(metadata, "title")?, "title", 200)?;
    let wasm = object(manifest, "wasmModule")?;
    if string(wasm, "path")? != "game.wasm"
        || string(wasm, "abi")? != "neo-grounds-wasm-component-v1"
        || string(wasm, "hashAlgorithm")? != "sha256"
        || string(wasm, "contentType")? != "application/wasm"
        || wasm.get("imports").and_then(Value::as_array).is_none()
        || wasm.get("exports").and_then(Value::as_array).is_none()
    {
        return Err(invalid("WASM artifact compatibility is unsupported"));
    }
    let assets = object(manifest, "assets")?;
    if string(object(assets, "manifest")?, "path")? != "assets.json" {
        return Err(invalid("Asset manifest path is invalid"));
    }
    let platform = object(manifest, "platformApi")?;
    let permissions = string_array(platform, "permissions")?;
    let allowed = [
        "achievements",
        "analytics",
        "identity",
        "leaderboards",
        "multiplayer_signals",
        "save_data",
    ];
    if permissions.windows(2).any(|pair| pair[0] >= pair[1])
        || permissions
            .iter()
            .any(|permission| !allowed.contains(&permission.as_str()))
    {
        return Err(invalid("Runtime permissions are unsupported or unsorted"));
    }
    let integrity = object(manifest, "integrity")?;
    validate_hash(string(integrity, "manifestHash")?, "manifest digest")?;
    if string(integrity, "manifestHashAlgorithm")? != "sha256" {
        return Err(invalid("Manifest digest algorithm is unsupported"));
    }
    let signature = object(integrity, "signature")?;
    if string(signature, "algorithm")? != "ed25519" || string(signature, "path")? != "package.sig" {
        return Err(invalid("Manifest signature metadata is unsupported"));
    }
    validate_identifier(string(signature, "keyId")?, "manifest signature key")?;
    validate_hash(
        string(signature, "signedManifestHash")?,
        "signed manifest digest",
    )
}

fn verify_manifest_artifacts(
    manifest: &Value,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), AppError> {
    let manifest = manifest.as_object().expect("validated manifest");
    verify_artifact(object(manifest, "wasmModule")?, files)?;
    let assets = object(manifest, "assets")?;
    verify_artifact(object(assets, "manifest")?, files)?;
    for asset in assets
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Runtime asset items must be an array"))?
    {
        verify_artifact(
            asset
                .as_object()
                .ok_or_else(|| invalid("Runtime asset must be an object"))?,
            files,
        )?;
    }
    Ok(())
}

fn verify_artifact(
    artifact: &Map<String, Value>,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), AppError> {
    let path = string(artifact, "path")?;
    validate_path(path)?;
    if string(artifact, "hashAlgorithm")? != "sha256" {
        return Err(invalid("Artifact digest algorithm is unsupported"));
    }
    validate_hash(string(artifact, "hash")?, "artifact digest")?;
    let bytes = required_file(files, path)?;
    if unsigned(artifact, "byteLength")? != bytes.len()
        || string(artifact, "hash")? != sha256(bytes)
    {
        return Err(invalid(&format!(
            "Artifact metadata does not match: {path}"
        )));
    }
    Ok(())
}

fn compute_manifest_hash(manifest: &Value) -> Result<String, AppError> {
    let mut normalized = manifest.clone();
    let integrity = normalized
        .get_mut("integrity")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Manifest integrity is invalid"))?;
    integrity.insert("manifestHash".into(), Value::String(EMPTY_HASH.into()));
    object_mut(integrity, "signature")?.insert(
        "signedManifestHash".into(),
        Value::String(EMPTY_HASH.into()),
    );
    Ok(sha256(&canonical_bytes(&normalized)?))
}

fn compute_content_digest(
    files: &BTreeMap<String, Vec<u8>>,
    manifest: &Value,
) -> Result<String, AppError> {
    let mut normalized_manifest = manifest.clone();
    let integrity = normalized_manifest
        .get_mut("integrity")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Manifest integrity is invalid"))?;
    object_mut(integrity, "signature")?.insert(
        "signedManifestHash".into(),
        Value::String(EMPTY_HASH.into()),
    );
    let normalized: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter(|(path, _)| path.as_str() != "package.sig")
        .map(|(path, bytes)| {
            if path == "runtime.json" {
                Ok((path.clone(), canonical_bytes(&normalized_manifest)?))
            } else {
                Ok((path.clone(), bytes.clone()))
            }
        })
        .collect::<Result<_, AppError>>()?;
    Ok(sha256(&create_archive(&normalized)?))
}

fn validate_creator_signature_shape(
    signature: &HarborGameCreatorSignatureEnvelope,
) -> Result<(), AppError> {
    if signature.format != "harbor-game-creator-signature"
        || signature.version != 1
        || signature.signature.algorithm != "ed25519"
        || signature.payload.domain != "harbor.game-package.v1"
        || signature.payload.version != 1
    {
        return Err(invalid("Creator signature shape is unsupported"));
    }
    validate_identifier(&signature.payload.creator_peer_id, "creator peer ID")?;
    validate_identifier(&signature.payload.game_id, "game ID")?;
    validate_identifier(&signature.payload.version_id, "version ID")?;
    validate_identifier(&signature.payload.request_id, "request ID")?;
    validate_hash(&signature.payload.creator_public_key, "creator public key")?;
    validate_hash(&signature.payload.package_digest, "package digest")?;
    validate_text(&signature.payload.title, "title", 200)
}

fn verify_creator_signature(envelope: &HarborGameCreatorSignatureEnvelope) -> Result<(), AppError> {
    let public_bytes: [u8; 32] = hex::decode(&envelope.payload.creator_public_key)
        .map_err(|_| invalid("Creator public key is not hexadecimal"))?
        .try_into()
        .map_err(|_| invalid("Creator public key length is invalid"))?;
    let key = VerifyingKey::from_bytes(&public_bytes)
        .map_err(|_| invalid("Creator public key is invalid"))?;
    let derived = CryptoService::derive_peer_id_from_verifying_key(&key)?;
    if derived != envelope.payload.creator_peer_id {
        return Err(invalid(
            "Creator PeerId does not derive from the public key",
        ));
    }
    let signature = Signature::from_slice(
        &hex::decode(&envelope.signature.value)
            .map_err(|_| invalid("Creator signature is not hexadecimal"))?,
    )
    .map_err(|_| invalid("Creator signature length is invalid"))?;
    let payload = serde_json::to_vec(&envelope.payload)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    key.verify(&payload, &signature)
        .map_err(|_| invalid("Creator signature is invalid"))
}

fn parse_canonical_json(bytes: &[u8], label: &str) -> Result<Value, AppError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid(&format!("{label} is not UTF-8")))?;
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid(&format!("{label} is not valid JSON")))?;
    validate_json_value(&value)?;
    let canonical = serde_json::to_string(&value)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    if canonical != text {
        return Err(invalid(&format!("{label} is not canonical JSON")));
    }
    Ok(value)
}

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, AppError> {
    validate_json_value(value)?;
    serde_json::to_vec(value).map_err(|error| AppError::Serialization(error.to_string()))
}

fn validate_json_value(value: &Value) -> Result<(), AppError> {
    match value {
        Value::Number(number) => {
            let valid = number
                .as_i64()
                .is_some_and(|value| value.unsigned_abs() <= 9_007_199_254_740_991)
                || number
                    .as_u64()
                    .is_some_and(|value| value <= 9_007_199_254_740_991);
            if !valid {
                return Err(invalid("JSON number is not a safe integer"));
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_json_value(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_json_value(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_file<'a>(
    files: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<&'a [u8], AppError> {
    files
        .get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(&format!("Archive is missing {path}")))
}

fn object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, AppError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(&format!("{key} must be an object")))
}

fn object_mut<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, AppError> {
    object
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid(&format!("{key} must be an object")))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AppError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("{key} must be a string")))
}

fn integer(object: &Map<String, Value>, key: &str) -> Result<i64, AppError> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(&format!("{key} must be an integer")))
}

fn unsigned(object: &Map<String, Value>, key: &str) -> Result<usize, AppError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| invalid(&format!("{key} must be an unsigned integer")))
}

fn string_array(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, AppError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(&format!("{key} must be an array")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(&format!("{key} values must be strings")))
        })
        .collect()
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str], label: &str) -> Result<(), AppError> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(&format!("{label} contains unsupported fields")));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), AppError> {
    if path.is_empty()
        || path.len() > 512
        || path.nfc().collect::<String>() != path
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid("Archive path is unsafe"));
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(invalid(&format!("Invalid {label}")));
    }
    Ok(())
}

fn validate_text(value: &str, label: &str, maximum_bytes: usize) -> Result<(), AppError> {
    if value.trim().is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control)
    {
        return Err(invalid(&format!("Invalid {label}")));
    }
    Ok(())
}

fn validate_hash(value: &str, label: &str) -> Result<(), AppError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(&format!("Invalid {label}")));
    }
    Ok(())
}

fn sha256(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

fn invalid(message: &str) -> AppError {
    AppError::InvalidData(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/harbor-game-v1/valid.harborgame");

    #[test]
    fn verifies_shared_golden_package() {
        let package = verify_game_package(FIXTURE).unwrap();
        assert_eq!(
            package.archive_digest,
            "c62b6398a6177a635b0049b669a0be0bd9222230d18e12403fce6c8ba1ae71c8"
        );
        assert_eq!(package.title, "Golden Runner");
        assert_eq!(package.permissions, vec!["save_data"]);
    }

    #[test]
    fn rejects_every_single_byte_archive_mutation() {
        for index in [0, 10, 15, 16, FIXTURE.len() / 2, FIXTURE.len() - 1] {
            let mut mutated = FIXTURE.to_vec();
            mutated[index] ^= 1;
            assert!(
                verify_game_package(&mutated).is_err(),
                "mutation {index} passed"
            );
        }
    }

    #[test]
    fn rejects_structural_manifest_identity_and_signature_attacks() {
        let files = read_archive(FIXTURE).unwrap();
        let base: Vec<_> = files
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect();

        let mut traversal = base.clone();
        traversal.push(("../escape".into(), b"bad".to_vec()));
        assert!(verify_game_package(&create_archive(&traversal).unwrap()).is_err());

        let mut duplicate = base.clone();
        duplicate.push(("game.wasm".into(), files["game.wasm"].clone()));
        assert!(verify_game_package(&create_archive(&duplicate).unwrap()).is_err());

        let mut undeclared = base.clone();
        undeclared.push(("extra.bin".into(), b"extra".to_vec()));
        assert!(verify_game_package(&create_archive(&undeclared).unwrap()).is_err());

        let mut incompatible_files = files.clone();
        let mut manifest = parse_canonical_json(&files["runtime.json"], "runtime.json").unwrap();
        manifest["compatibility"]["runtimeAbi"] = Value::String("unknown-abi".into());
        incompatible_files.insert("runtime.json".into(), canonical_bytes(&manifest).unwrap());
        assert!(verify_game_package(
            &create_archive(&incompatible_files.into_iter().collect::<Vec<_>>()).unwrap()
        )
        .is_err());

        for field in ["creatorPeerId", "signature"] {
            let mut attacked = files.clone();
            let mut envelope = parse_canonical_json(&files["package.sig"], "package.sig").unwrap();
            if field == "creatorPeerId" {
                envelope["payload"]["creatorPeerId"] = Value::String("12D3KooWsubstitution".into());
            } else {
                let signature = envelope["signature"]["value"].as_str().unwrap().to_string();
                envelope["signature"]["value"] = Value::String(format!(
                    "{}{}",
                    if &signature[..1] == "0" { "1" } else { "0" },
                    &signature[1..]
                ));
            }
            attacked.insert("package.sig".into(), canonical_bytes(&envelope).unwrap());
            assert!(verify_game_package(
                &create_archive(&attacked.into_iter().collect::<Vec<_>>()).unwrap()
            )
            .is_err());
        }
    }
}
