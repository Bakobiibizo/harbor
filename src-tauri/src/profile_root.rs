use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRoot(PathBuf);
impl ProfileRoot {
    pub fn resolve(default: &Path, custom: Option<PathBuf>, profile: Option<&str>) -> Self {
        Self(custom.unwrap_or_else(|| {
            profile
                .map(|p| default.join(format!("profile-{p}")))
                .unwrap_or_else(|| default.to_path_buf())
        }))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
    pub fn database(&self) -> PathBuf {
        self.0.join("harbor.db")
    }
    pub fn logs(&self) -> PathBuf {
        self.0.join("logs")
    }
    pub fn webview(&self) -> PathBuf {
        self.0.join("webview")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        services::{AccountsService, MediaStorageService},
    };
    use std::sync::Arc;
    #[test]
    fn distinct_roots_cannot_enumerate_or_mutate_each_other() {
        let temp = tempfile::tempdir().unwrap();
        let a = ProfileRoot::resolve(temp.path(), Some(temp.path().join("a")), None);
        let b = ProfileRoot::resolve(temp.path(), Some(temp.path().join("b")), None);
        let aa = AccountsService::new(a.path().to_path_buf());
        let bb = AccountsService::new(b.path().to_path_buf());
        aa.register_account("peer-a".into(), "Alice".into(), None, None)
            .unwrap();
        assert!(bb.list_accounts().unwrap().is_empty());
        let adb = Arc::new(Database::new(a.database()).unwrap());
        let bdb = Arc::new(Database::new(b.database()).unwrap());
        let am = MediaStorageService::new(a.path(), adb).unwrap();
        let bm = MediaStorageService::new(b.path(), bdb).unwrap();
        let hash = am.store_media(b"private-a", "image/png").unwrap();
        assert!(am.has_media(&hash));
        assert!(!bm.has_media(&hash));
        assert_ne!(a.logs(), b.logs());
        assert_ne!(a.webview(), b.webview());
    }
    #[test]
    fn same_root_preserves_multi_account_registry() {
        let temp = tempfile::tempdir().unwrap();
        let root = ProfileRoot::resolve(temp.path(), None, None);
        let accounts = AccountsService::new(root.path().to_path_buf());
        let a = accounts
            .register_account("peer-a".into(), "Alice".into(), None, None)
            .unwrap();
        let b = accounts
            .register_account("peer-b".into(), "Bob".into(), None, None)
            .unwrap();
        assert_eq!(accounts.list_accounts().unwrap().len(), 2);
        accounts.set_active_account(&a.id).unwrap();
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().peer_id,
            "peer-a"
        );
        accounts.set_active_account(&b.id).unwrap();
        assert_eq!(
            accounts.get_active_account().unwrap().unwrap().peer_id,
            "peer-b"
        );
    }
}
