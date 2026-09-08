use crate::{
    db::Database,
    error::AppError,
    services::{
        extract_verified_game_files, verify_game_package, verify_store_approval, StoreApproval,
        VerifiedGamePackage,
    },
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const MAX_GAME_LIBRARY_BYTES: i64 = 128 * 8 * 1024 * 1024;
pub const MAX_GAME_SAVE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameInstallation {
    pub archive_digest: String,
    pub byte_length: i64,
    pub creator_peer_id: String,
    pub game_id: String,
    pub installed_at: i64,
    pub last_played_at: Option<i64>,
    pub package_digest: String,
    pub permissions: Vec<String>,
    pub play_count: i64,
    pub source: String,
    pub store_approval: Option<StoreApproval>,
    pub title: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDiscoveryResult {
    pub file_name: String,
    pub installation: Option<GameInstallation>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDiscoveryPreview {
    pub error: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub package: Option<VerifiedGamePackage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRuntimeBundle {
    pub asset_manifest: serde_json::Value,
    pub assets: BTreeMap<String, Vec<u8>>,
    pub game_id: String,
    pub manifest: serde_json::Value,
    pub version_id: String,
    pub wasm_bytes: Vec<u8>,
}

pub struct GameLibraryService {
    database: Arc<Database>,
    package_root: PathBuf,
}

impl GameLibraryService {
    pub fn new(profile_root: &Path, database: Arc<Database>) -> Result<Self, AppError> {
        let package_root = profile_root.join("games").join("packages");
        fs::create_dir_all(&package_root)?;
        if fs::symlink_metadata(&package_root)?
            .file_type()
            .is_symlink()
        {
            return Err(validation("Managed game package root cannot be a symlink"));
        }
        Ok(Self {
            database,
            package_root,
        })
    }

    pub fn inspect_package(&self, source_path: &Path) -> Result<VerifiedGamePackage, AppError> {
        verify_game_package(&read_regular_package(source_path)?)
    }

    pub fn import_manual(
        &self,
        source_path: &Path,
        approved_permissions: &[String],
        now: i64,
    ) -> Result<GameInstallation, AppError> {
        let bytes = read_regular_package(source_path)?;
        self.install_bytes(&bytes, approved_permissions, "manual", None, None, now)
    }

    pub fn configure_discovery_folder(&self, folder: &Path) -> Result<(), AppError> {
        let metadata = fs::symlink_metadata(folder)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(validation(
                "Games discovery folder must be a real directory, not a symlink",
            ));
        }
        let canonical = folder.canonicalize()?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_library_settings SET discovery_folder=?1 WHERE id=1",
                [canonical.to_string_lossy().as_ref()],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn trust_store_public_key(&self, public_key: &str) -> Result<(), AppError> {
        if public_key.len() != 88
            || !public_key
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(validation("Store approval public key format is invalid"));
        }
        self.database
            .with_connection_mut(|connection| {
                let transaction = connection.transaction()?;
                let existing: Option<String> = transaction.query_row(
                    "SELECT trusted_store_public_key FROM game_library_settings WHERE id=1",
                    [],
                    |row| row.get(0),
                )?;
                if existing
                    .as_deref()
                    .is_some_and(|existing| existing != public_key)
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                transaction.execute(
                    "UPDATE game_library_settings SET trusted_store_public_key=?1 WHERE id=1",
                    [public_key],
                )?;
                transaction.commit()
            })
            .map_err(|error| match error {
                rusqlite::Error::InvalidQuery => AppError::PermissionDenied(
                    "Neo Grounds store approval key changed; explicit trust reset is required"
                        .into(),
                ),
                other => other.into(),
            })
    }

    pub fn configured_discovery_folder(&self) -> Result<Option<PathBuf>, AppError> {
        self.database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT discovery_folder FROM game_library_settings WHERE id=1",
                    [],
                    |row| row.get::<_, Option<String>>(0),
                )
            })
            .map(|value| value.map(PathBuf::from))
            .map_err(Into::into)
    }

    pub fn inspect_discovery_folder(&self) -> Result<Vec<GameDiscoveryPreview>, AppError> {
        let folder = self
            .configured_discovery_folder()?
            .ok_or_else(|| validation("Configure a games discovery folder first"))?;
        let metadata = fs::symlink_metadata(&folder)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(validation(
                "Configured games discovery folder is no longer a real directory",
            ));
        }
        let mut entries: Vec<_> = fs::read_dir(folder)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        Ok(entries
            .into_iter()
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("harborgame")
            })
            .map(|entry| {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().into_owned();
                match self.inspect_package(&path) {
                    Ok(package) => GameDiscoveryPreview {
                        error: None,
                        file_name,
                        file_path: path.to_string_lossy().into_owned(),
                        package: Some(package),
                    },
                    Err(error) => GameDiscoveryPreview {
                        error: Some(error.to_string()),
                        file_name,
                        file_path: path.to_string_lossy().into_owned(),
                        package: None,
                    },
                }
            })
            .collect())
    }

    pub fn discover_and_install(
        &self,
        approved_permissions: &[String],
        now: i64,
    ) -> Result<Vec<GameDiscoveryResult>, AppError> {
        let folder = self
            .configured_discovery_folder()?
            .ok_or_else(|| validation("Configure a games discovery folder first"))?;
        let metadata = fs::symlink_metadata(&folder)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(validation(
                "Configured games discovery folder is no longer a real directory",
            ));
        }
        let mut entries: Vec<_> = fs::read_dir(folder)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut results = Vec::new();
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("harborgame") {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let outcome = read_regular_package(&path).and_then(|bytes| {
                self.install_bytes(&bytes, approved_permissions, "folder", None, None, now)
            });
            match outcome {
                Ok(installation) => results.push(GameDiscoveryResult {
                    error: None,
                    file_name,
                    installation: Some(installation),
                }),
                Err(error) => results.push(GameDiscoveryResult {
                    error: Some(error.to_string()),
                    file_name,
                    installation: None,
                }),
            }
        }
        Ok(results)
    }

    pub fn install_store_package(
        &self,
        bytes: &[u8],
        approved_permissions: &[String],
        approval: StoreApproval,
        trusted_store_public_key: &str,
        now: i64,
    ) -> Result<GameInstallation, AppError> {
        self.install_bytes(
            bytes,
            approved_permissions,
            "store",
            Some(approval),
            Some(trusted_store_public_key),
            now,
        )
    }

    fn install_bytes(
        &self,
        bytes: &[u8],
        approved_permissions: &[String],
        source: &str,
        approval: Option<StoreApproval>,
        trusted_store_public_key: Option<&str>,
        now: i64,
    ) -> Result<GameInstallation, AppError> {
        let package = verify_game_package(bytes)?;
        validate_permissions(&package, approved_permissions)?;
        match (&approval, trusted_store_public_key) {
            (Some(approval), Some(trusted_key)) if source == "store" => {
                verify_store_approval(approval, &package, trusted_key)?
            }
            (None, None) if source != "store" => {}
            _ => {
                return Err(validation(
                    "Store installation requires one trusted approval",
                ))
            }
        }
        self.enforce_library_quota(&package)?;
        let final_path = self.package_path(&package.archive_digest);
        write_immutable(&final_path, bytes, &package.archive_digest)?;
        let installation = installation_from_package(package, source, approval, now);
        let permissions_json = serde_json::to_string(&installation.permissions)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let approval_json = installation
            .store_approval
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        self.database.with_connection_mut_result(|connection| {
            let transaction = connection.transaction()?;
            let total: i64 = transaction.query_row(
                "SELECT COALESCE(SUM(byte_length),0) FROM game_installations",
                [],
                |row| row.get(0),
            )?;
            let replaced: i64 = transaction
                .query_row(
                    "SELECT byte_length FROM game_installations WHERE game_id=?1",
                    [&installation.game_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if total
                .saturating_sub(replaced)
                .saturating_add(installation.byte_length)
                > MAX_GAME_LIBRARY_BYTES
            {
                return Err(validation("Game library quota exceeded"));
            }
            transaction.execute(
                "INSERT INTO game_installations(
                    game_id,version_id,title,creator_peer_id,archive_digest,package_digest,
                    byte_length,permissions_json,source,store_approval_json,installed_at,
                    last_played_at,play_count
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL,0)
                 ON CONFLICT(game_id) DO UPDATE SET
                    version_id=excluded.version_id,
                    title=excluded.title,
                    creator_peer_id=excluded.creator_peer_id,
                    archive_digest=excluded.archive_digest,
                    package_digest=excluded.package_digest,
                    byte_length=excluded.byte_length,
                    permissions_json=excluded.permissions_json,
                    source=excluded.source,
                    store_approval_json=excluded.store_approval_json,
                    installed_at=excluded.installed_at",
                rusqlite::params![
                    installation.game_id,
                    installation.version_id,
                    installation.title,
                    installation.creator_peer_id,
                    installation.archive_digest,
                    installation.package_digest,
                    installation.byte_length,
                    permissions_json,
                    installation.source,
                    approval_json,
                    installation.installed_at,
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })?;
        self.get(&installation.game_id)
    }

    fn enforce_library_quota(&self, package: &VerifiedGamePackage) -> Result<(), AppError> {
        let (total, replaced): (i64, i64) = self.database.with_connection(|connection| {
            let total = connection.query_row(
                "SELECT COALESCE(SUM(byte_length),0) FROM game_installations",
                [],
                |row| row.get(0),
            )?;
            let replaced = connection
                .query_row(
                    "SELECT byte_length FROM game_installations WHERE game_id=?1",
                    [&package.game_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            Ok((total, replaced))
        })?;
        let projected = total
            .saturating_sub(replaced)
            .saturating_add(package.byte_length as i64);
        if projected > MAX_GAME_LIBRARY_BYTES {
            return Err(validation("Game library quota exceeded"));
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<GameInstallation>, AppError> {
        self.database
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT game_id,version_id,title,creator_peer_id,archive_digest,
                            package_digest,byte_length,permissions_json,source,
                            store_approval_json,installed_at,last_played_at,play_count
                     FROM game_installations ORDER BY title,game_id",
                )?;
                let installations = statement
                    .query_map([], installation_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(installations)
            })
            .map_err(Into::into)
    }

    pub fn get(&self, game_id: &str) -> Result<GameInstallation, AppError> {
        validate_identifier(game_id, "game ID")?;
        self.database
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT game_id,version_id,title,creator_peer_id,archive_digest,
                            package_digest,byte_length,permissions_json,source,
                            store_approval_json,installed_at,last_played_at,play_count
                     FROM game_installations WHERE game_id=?1",
                    [game_id],
                    installation_from_row,
                )
            })
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound("Installed game was not found".into())
                }
                other => other.into(),
            })
    }

    pub fn load_runtime_bundle(&self, game_id: &str) -> Result<GameRuntimeBundle, AppError> {
        let installation = self.get(game_id)?;
        let path = self.verified_package_path(game_id)?;
        let bytes = fs::read(path)?;
        let mut files = extract_verified_game_files(&bytes)?;
        let manifest = serde_json::from_slice(
            &files
                .remove("runtime.json")
                .ok_or_else(|| AppError::InvalidData("Runtime manifest disappeared".into()))?,
        )
        .map_err(|error| AppError::Serialization(error.to_string()))?;
        let asset_manifest = serde_json::from_slice(
            &files
                .remove("assets.json")
                .ok_or_else(|| AppError::InvalidData("Asset manifest disappeared".into()))?,
        )
        .map_err(|error| AppError::Serialization(error.to_string()))?;
        let wasm_bytes = files
            .remove("game.wasm")
            .ok_or_else(|| AppError::InvalidData("WASM module disappeared".into()))?;
        files.remove("package.sig");
        Ok(GameRuntimeBundle {
            asset_manifest,
            assets: files,
            game_id: installation.game_id,
            manifest,
            version_id: installation.version_id,
            wasm_bytes,
        })
    }

    pub fn verified_package_path(&self, game_id: &str) -> Result<PathBuf, AppError> {
        let installation = self.get(game_id)?;
        let path = self.package_path(&installation.archive_digest);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AppError::InvalidData(
                "Managed game package is not a regular immutable file".into(),
            ));
        }
        let bytes = fs::read(&path)?;
        let package = verify_game_package(&bytes)?;
        if package.archive_digest != installation.archive_digest
            || package.package_digest != installation.package_digest
        {
            return Err(AppError::InvalidData(
                "Managed game package no longer matches its installation record".into(),
            ));
        }
        Ok(path)
    }

    pub fn uninstall(&self, game_id: &str) -> Result<(), AppError> {
        validate_identifier(game_id, "game ID")?;
        let changed = self.database.with_connection(|connection| {
            connection.execute("DELETE FROM game_installations WHERE game_id=?1", [game_id])
        })?;
        if changed == 0 {
            return Err(AppError::NotFound("Installed game was not found".into()));
        }
        Ok(())
    }

    pub fn write_save(
        &self,
        game_id: &str,
        slot: &str,
        data: &[u8],
        now: i64,
    ) -> Result<(), AppError> {
        self.get(game_id)?;
        validate_identifier(slot, "save slot")?;
        if data.len() > MAX_GAME_SAVE_BYTES {
            return Err(validation("Game save exceeds the 1 MiB limit"));
        }
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO game_saves(game_id,slot,data,updated_at) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(game_id,slot) DO UPDATE SET data=excluded.data,updated_at=excluded.updated_at",
                rusqlite::params![game_id, slot, data, now],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn read_save(&self, game_id: &str, slot: &str) -> Result<Option<Vec<u8>>, AppError> {
        validate_identifier(game_id, "game ID")?;
        validate_identifier(slot, "save slot")?;
        self.database
            .with_connection(|connection| {
                match connection.query_row(
                    "SELECT data FROM game_saves WHERE game_id=?1 AND slot=?2",
                    rusqlite::params![game_id, slot],
                    |row| row.get(0),
                ) {
                    Ok(data) => Ok(Some(data)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(error) => Err(error),
                }
            })
            .map_err(Into::into)
    }

    pub fn delete_saves(&self, game_id: &str) -> Result<usize, AppError> {
        validate_identifier(game_id, "game ID")?;
        self.database
            .with_connection(|connection| {
                connection.execute("DELETE FROM game_saves WHERE game_id=?1", [game_id])
            })
            .map_err(Into::into)
    }

    pub fn record_launch(&self, game_id: &str, now: i64) -> Result<(), AppError> {
        validate_identifier(game_id, "game ID")?;
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_installations SET last_played_at=?2,play_count=play_count+1 WHERE game_id=?1",
                rusqlite::params![game_id, now],
            )
        })?;
        if changed == 0 {
            return Err(AppError::NotFound("Installed game was not found".into()));
        }
        Ok(())
    }

    fn package_path(&self, archive_digest: &str) -> PathBuf {
        self.package_root
            .join(format!("{archive_digest}.harborgame"))
    }
}

fn installation_from_package(
    package: VerifiedGamePackage,
    source: &str,
    store_approval: Option<StoreApproval>,
    now: i64,
) -> GameInstallation {
    GameInstallation {
        archive_digest: package.archive_digest,
        byte_length: package.byte_length as i64,
        creator_peer_id: package.creator_peer_id,
        game_id: package.game_id,
        installed_at: now,
        last_played_at: None,
        package_digest: package.package_digest,
        permissions: package.permissions,
        play_count: 0,
        source: source.into(),
        store_approval,
        title: package.title,
        version_id: package.version_id,
    }
}

fn installation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GameInstallation> {
    let permissions_json: String = row.get(7)?;
    let approval_json: Option<String> = row.get(9)?;
    let permissions = serde_json::from_str(&permissions_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            permissions_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let store_approval = approval_json
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    value.len(),
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()?;
    Ok(GameInstallation {
        game_id: row.get(0)?,
        version_id: row.get(1)?,
        title: row.get(2)?,
        creator_peer_id: row.get(3)?,
        archive_digest: row.get(4)?,
        package_digest: row.get(5)?,
        byte_length: row.get(6)?,
        permissions,
        source: row.get(8)?,
        store_approval,
        installed_at: row.get(10)?,
        last_played_at: row.get(11)?,
        play_count: row.get(12)?,
    })
}

fn read_regular_package(path: &Path) -> Result<Vec<u8>, AppError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(validation(
            "Game package must be a regular file, not a symlink or special file",
        ));
    }
    if metadata.len() as usize > crate::services::MAX_HARBOR_GAME_BYTES {
        return Err(validation("Game package exceeds the 8 MiB limit"));
    }
    fs::read(path).map_err(Into::into)
}

fn write_immutable(path: &Path, bytes: &[u8], expected_digest: &str) -> Result<(), AppError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(AppError::InvalidData(
                    "Managed package digest path is not a regular file".into(),
                ));
            }
            let existing = fs::read(path)?;
            let actual = hex::encode(sha2::Sha256::digest(&existing));
            if actual != expected_digest || existing != bytes {
                return Err(AppError::InvalidData(
                    "Managed package digest path contains different bytes".into(),
                ));
            }
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn validate_permissions(
    package: &VerifiedGamePackage,
    approved_permissions: &[String],
) -> Result<(), AppError> {
    if approved_permissions != package.permissions {
        return Err(validation(
            "Approved permissions must exactly match the package declaration",
        ));
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
        return Err(validation(&format!("Invalid {label}")));
    }
    Ok(())
}

fn validation(message: &str) -> AppError {
    AppError::Validation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/harbor-game-v1/valid.harborgame");

    fn service() -> (tempfile::TempDir, GameLibraryService) {
        let root = tempfile::tempdir().unwrap();
        let database = Arc::new(Database::new(root.path().join("harbor.db")).unwrap());
        let service = GameLibraryService::new(root.path(), database).unwrap();
        (root, service)
    }

    #[test]
    fn manual_import_copies_verified_package_and_preserves_saves_on_uninstall() {
        let (root, service) = service();
        let source = root.path().join("fixture.harborgame");
        fs::write(&source, FIXTURE).unwrap();
        let installed = service
            .import_manual(&source, &["save_data".into()], 10)
            .unwrap();
        assert_ne!(
            service.verified_package_path(&installed.game_id).unwrap(),
            source
        );
        let bundle = service.load_runtime_bundle(&installed.game_id).unwrap();
        assert_eq!(&bundle.wasm_bytes[..4], b"\0asm");
        assert!(bundle.assets.is_empty());
        service
            .write_save(&installed.game_id, "main", b"save", 11)
            .unwrap();
        service.record_launch(&installed.game_id, 12).unwrap();
        assert_eq!(service.get(&installed.game_id).unwrap().play_count, 1);
        service.uninstall(&installed.game_id).unwrap();
        assert_eq!(
            service.read_save(&installed.game_id, "main").unwrap(),
            Some(b"save".to_vec())
        );
        assert_eq!(service.delete_saves(&installed.game_id).unwrap(), 1);
    }

    #[test]
    fn discovery_is_non_recursive_and_rejects_symlinks_and_tampering() {
        let (root, service) = service();
        let discovery = root.path().join("discovery");
        fs::create_dir(&discovery).unwrap();
        fs::write(discovery.join("valid.harborgame"), FIXTURE).unwrap();
        let mut tampered = FIXTURE.to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        fs::write(discovery.join("tampered.harborgame"), tampered).unwrap();
        fs::create_dir(discovery.join("nested")).unwrap();
        fs::write(discovery.join("nested").join("ignored.harborgame"), FIXTURE).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            discovery.join("valid.harborgame"),
            discovery.join("link.harborgame"),
        )
        .unwrap();
        service.configure_discovery_folder(&discovery).unwrap();
        let results = service
            .discover_and_install(&["save_data".into()], 10)
            .unwrap();
        assert_eq!(
            results
                .iter()
                .filter(|result| result.installation.is_some())
                .count(),
            1
        );
        assert!(results
            .iter()
            .any(|result| result.file_name == "tampered.harborgame" && result.error.is_some()));
        #[cfg(unix)]
        assert!(results
            .iter()
            .any(|result| result.file_name == "link.harborgame" && result.error.is_some()));
        assert_eq!(service.list().unwrap().len(), 1);
    }

    #[test]
    fn wrong_permission_approval_and_failed_update_preserve_installed_version() {
        let (root, service) = service();
        let source = root.path().join("fixture.harborgame");
        fs::write(&source, FIXTURE).unwrap();
        assert!(service.import_manual(&source, &[], 10).is_err());
        let installed = service
            .import_manual(&source, &["save_data".into()], 10)
            .unwrap();
        let mut tampered = FIXTURE.to_vec();
        let middle = tampered.len() / 2;
        tampered[middle] ^= 1;
        fs::write(&source, tampered).unwrap();
        assert!(service
            .import_manual(&source, &["save_data".into()], 11)
            .is_err());
        assert_eq!(
            service.get(&installed.game_id).unwrap().archive_digest,
            installed.archive_digest
        );
    }

    #[test]
    fn separate_profile_roots_cannot_see_installations_or_saves() {
        let first_root = tempfile::tempdir().unwrap();
        let second_root = tempfile::tempdir().unwrap();
        let first = GameLibraryService::new(
            first_root.path(),
            Arc::new(Database::new(first_root.path().join("harbor.db")).unwrap()),
        )
        .unwrap();
        let second = GameLibraryService::new(
            second_root.path(),
            Arc::new(Database::new(second_root.path().join("harbor.db")).unwrap()),
        )
        .unwrap();
        let source = first_root.path().join("fixture.harborgame");
        fs::write(&source, FIXTURE).unwrap();
        let installed = first
            .import_manual(&source, &["save_data".into()], 10)
            .unwrap();
        first
            .write_save(&installed.game_id, "main", b"private", 11)
            .unwrap();
        assert!(second.list().unwrap().is_empty());
        assert_eq!(second.read_save(&installed.game_id, "main").unwrap(), None);
    }

    #[test]
    fn store_install_requires_a_matching_trusted_approval() {
        let (_root, service) = service();
        let package = verify_game_package(FIXTURE).unwrap();
        let key = SigningKey::generate(&mut OsRng);
        let mut public_der = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        public_der.extend_from_slice(&key.verifying_key().to_bytes());
        let public_key = hex::encode(public_der);
        let payload = crate::services::game_package::StoreApprovalPayload {
            approved_at: 10,
            archive_digest: package.archive_digest,
            creator_peer_id: package.creator_peer_id,
            domain: "neo-grounds.harbor-store.approval.v1".into(),
            game_id: package.game_id,
            reviewer_id: "operator-1".into(),
            version: 1,
            version_id: package.version_id,
        };
        let signature = hex::encode(key.sign(&serde_json::to_vec(&payload).unwrap()).to_bytes());
        service.trust_store_public_key(&public_key).unwrap();
        assert!(service.trust_store_public_key(&"00".repeat(44)).is_err());
        let approval = StoreApproval {
            algorithm: "ed25519".into(),
            payload,
            public_key: public_key.clone(),
            signature,
        };
        assert!(service
            .install_store_package(
                FIXTURE,
                &["save_data".into()],
                approval.clone(),
                &"00".repeat(44),
                11,
            )
            .is_err());
        assert!(service
            .install_store_package(FIXTURE, &["save_data".into()], approval, &public_key, 11,)
            .is_ok());
    }

    #[test]
    fn save_and_library_limits_fail_without_mutating_attempted_state() {
        let (root, quota_service) = service();
        quota_service
            .database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO game_installations(
                        game_id,version_id,title,creator_peer_id,archive_digest,package_digest,
                        byte_length,permissions_json,source,installed_at,play_count
                     ) VALUES('existing','v1','Existing','peer','a','b',?1,'[]','manual',1,0)",
                    [MAX_GAME_LIBRARY_BYTES],
                )?;
                Ok(())
            })
            .unwrap();
        let source = root.path().join("fixture.harborgame");
        fs::write(&source, FIXTURE).unwrap();
        assert!(quota_service
            .import_manual(&source, &["save_data".into()], 10)
            .is_err());
        assert!(matches!(
            quota_service.get("game_golden_runner"),
            Err(AppError::NotFound(_))
        ));

        let (save_root, save_service) = service();
        let save_source = save_root.path().join("fixture.harborgame");
        fs::write(&save_source, FIXTURE).unwrap();
        let installed = save_service
            .import_manual(&save_source, &["save_data".into()], 10)
            .unwrap();
        assert!(save_service
            .write_save(
                &installed.game_id,
                "main",
                &vec![0; MAX_GAME_SAVE_BYTES + 1],
                11
            )
            .is_err());
        assert_eq!(
            save_service.read_save(&installed.game_id, "main").unwrap(),
            None
        );
    }
}
