//! Fail-closed storage for relay identity keys.

use libp2p::identity::Keypair;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityKeyError {
    #[error("identity key operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("refusing unsafe identity key {path}: {reason}")]
    Unsafe { path: PathBuf, reason: String },
    #[error("identity key {path} is not a valid libp2p key: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

fn io_error(path: &Path, source: io::Error) -> IdentityKeyError {
    IdentityKeyError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn validate_unix_metadata_fields(
    path: &Path,
    mode: u32,
    owner_uid: u32,
    effective_uid: u32,
) -> Result<(), IdentityKeyError> {
    if owner_uid != effective_uid {
        return Err(IdentityKeyError::Unsafe {
            path: path.to_path_buf(),
            reason: format!("owner uid {owner_uid} does not match process uid {effective_uid}"),
        });
    }
    let permissions = mode & 0o777;
    if !matches!(permissions, 0o400 | 0o600) {
        return Err(IdentityKeyError::Unsafe {
            path: path.to_path_buf(),
            reason: format!(
                "mode {permissions:04o} must be 0400 or 0600 with no group or other access"
            ),
        });
    }
    Ok(())
}

fn validate_identity_key_path(path: &Path) -> Result<(), IdentityKeyError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.file_type().is_file() {
        return Err(IdentityKeyError::Unsafe {
            path: path.to_path_buf(),
            reason: "path must be a regular file and must not be a symbolic link".into(),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        // SAFETY: geteuid has no preconditions and does not dereference pointers.
        let effective_uid = unsafe { libc::geteuid() };
        validate_unix_metadata_fields(path, metadata.mode(), metadata.uid(), effective_uid)?;
    }

    Ok(())
}

pub fn load_identity_key(path: &Path) -> Result<Keypair, IdentityKeyError> {
    validate_identity_key_path(path)?;
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    Keypair::from_protobuf_encoding(&bytes).map_err(|error| IdentityKeyError::Invalid {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}

fn create_identity_key(path: &Path, key: &Keypair) -> Result<(), IdentityKeyError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("identity.key");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut temporary = options
            .open(&temporary_path)
            .map_err(|source| io_error(&temporary_path, source))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|source| io_error(&temporary_path, source))?;
        }
        temporary
            .write_all(
                &key.to_protobuf_encoding()
                    .map_err(|error| IdentityKeyError::Invalid {
                        path: path.to_path_buf(),
                        reason: error.to_string(),
                    })?,
            )
            .map_err(|source| io_error(&temporary_path, source))?;
        temporary
            .sync_all()
            .map_err(|source| io_error(&temporary_path, source))?;
        drop(temporary);

        // A hard link publishes the fully written inode atomically and, unlike
        // rename, refuses to replace a key that appeared concurrently.
        fs::hard_link(&temporary_path, path).map_err(|source| io_error(path, source))?;
        validate_identity_key_path(path)
    })();

    let _ = fs::remove_file(&temporary_path);
    result
}

pub fn load_or_generate_identity(path: &Path) -> Result<Keypair, IdentityKeyError> {
    match fs::symlink_metadata(path) {
        Ok(_) => load_identity_key(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let key = Keypair::generate_ed25519();
            create_identity_key(path, &key)?;
            Ok(key)
        }
        Err(source) => Err(io_error(path, source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_an_atomic_owner_only_key_and_reloads_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("id.key");
        let created = load_or_generate_identity(&path).unwrap();
        let loaded = load_or_generate_identity(&path).unwrap();

        assert_eq!(created.public().to_peer_id(), loaded.public().to_peer_id());
        assert_eq!(
            fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1,
            "temporary key files must be removed after atomic publication"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_keys_with_group_or_other_access() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("id.key");
        let key = Keypair::generate_ed25519();
        fs::write(&path, key.to_protobuf_encoding().unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let error = load_identity_key(&path).unwrap_err().to_string();
        assert!(error.contains("refusing unsafe identity key"));
        assert!(error.contains("must be 0400 or 0600"));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(load_identity_key(&path)
            .unwrap_err()
            .to_string()
            .contains("must be 0400 or 0600"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_mismatched_owners() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.key");
        let link = directory.path().join("id.key");
        fs::write(&target, b"not relevant").unwrap();
        symlink(&target, &link).unwrap();
        assert!(load_identity_key(&link)
            .unwrap_err()
            .to_string()
            .contains("must not be a symbolic link"));

        assert!(validate_unix_metadata_fields(&target, 0o100600, 1000, 1001)
            .unwrap_err()
            .to_string()
            .contains("does not match process uid"));
    }
}
