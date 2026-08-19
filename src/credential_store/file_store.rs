// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::credential_store::{CredentialStore, CredentialStoreError};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

const XDG_RUNTIME_DIR_ENV: &str = "XDG_RUNTIME_DIR";

/// Credential store backed by a plain file, used as a fallback when no OS
/// keyring is available. Mirrors kfactory's `FileCredentialStore`, storing the
/// token under `$XDG_RUNTIME_DIR/gitlance/application_token` (cleared on
/// logout/reboot).
pub struct FileCredentialStore {
    token_path: PathBuf,
}

impl FileCredentialStore {
    /// Creates a new file-backed store, failing if `XDG_RUNTIME_DIR` is unset.
    pub fn new() -> Result<Self, CredentialStoreError> {
        let runtime_dir = std::env::var(XDG_RUNTIME_DIR_ENV).map_err(|_| {
            CredentialStoreError::InitStore(format!("{} is not set", XDG_RUNTIME_DIR_ENV))
        })?;

        Ok(Self {
            token_path: PathBuf::from(runtime_dir)
                .join("gitlance")
                .join("application_token"),
        })
    }

    #[cfg(test)]
    fn with_path(token_path: PathBuf) -> Self {
        Self { token_path }
    }
}

impl CredentialStore for FileCredentialStore {
    fn get_name(&self) -> &str {
        "file"
    }

    fn read_token(&self) -> Result<Option<String>, CredentialStoreError> {
        match fs::read_to_string(&self.token_path) {
            Ok(token) => Ok(Some(token)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CredentialStoreError::TokenRead(e.to_string())),
        }
    }

    fn write_token(&self, token: &str) -> Result<(), CredentialStoreError> {
        if let Some(parent) = self.token_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CredentialStoreError::TokenWrite(e.to_string()))?;
        }

        // Create the file with owner-only permissions from the start (rather
        // than writing with default permissions and chmod'ing afterwards),
        // since this file holds a live API key and the write-then-chmod
        // sequence leaves a brief window where it may be readable by
        // group/other depending on umask.
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.token_path)
                .map_err(|e| CredentialStoreError::TokenWrite(e.to_string()))?;

            file.write_all(token.as_bytes())
                .map_err(|e| CredentialStoreError::TokenWrite(e.to_string()))?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&self.token_path, token)
                .map_err(|e| CredentialStoreError::TokenWrite(e.to_string()))?;
        }

        Ok(())
    }

    fn delete_token(&self) -> Result<(), CredentialStoreError> {
        match fs::remove_file(&self.token_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CredentialStoreError::TokenDelete(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_token_missing_returns_none() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store = FileCredentialStore::with_path(temp_dir.path().join("application_token"));

        assert_eq!(store.read_token().expect("read_token failed"), None);
    }

    #[test]
    fn test_write_then_read_token_round_trip() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store = FileCredentialStore::with_path(temp_dir.path().join("application_token"));

        store
            .write_token("sk-test-token")
            .expect("write_token failed");
        assert_eq!(
            store.read_token().expect("read_token failed"),
            Some("sk-test-token".to_string())
        );
    }

    #[test]
    fn test_write_token_creates_parent_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store =
            FileCredentialStore::with_path(temp_dir.path().join("nested/dir/application_token"));

        store
            .write_token("sk-test-token")
            .expect("write_token failed");
        assert_eq!(
            store.read_token().expect("read_token failed"),
            Some("sk-test-token".to_string())
        );
    }

    #[test]
    fn test_delete_token_removes_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store = FileCredentialStore::with_path(temp_dir.path().join("application_token"));

        store
            .write_token("sk-test-token")
            .expect("write_token failed");
        store.delete_token().expect("delete_token failed");
        assert_eq!(store.read_token().expect("read_token failed"), None);
    }

    #[test]
    fn test_delete_token_missing_is_ok() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store = FileCredentialStore::with_path(temp_dir.path().join("application_token"));

        assert!(store.delete_token().is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_write_token_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let store = FileCredentialStore::with_path(temp_dir.path().join("application_token"));

        store
            .write_token("sk-test-token")
            .expect("write_token failed");
        let mode = fs::metadata(&store.token_path)
            .expect("metadata failed")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
