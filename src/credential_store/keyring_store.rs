// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::credential_store::{CredentialStore, CredentialStoreError};
use keyring::Entry;

/// Service/username under which the token is stored in the OS keyring,
/// analogous to kfactory's `SERVICE_NAME = "kernelfactory-cli"`.
const SERVICE_NAME: &str = "gitlance-cli";
const CLI_USERNAME: &str = "gitlance-cli-user";

/// Narrow interface over the OS keyring operations we need. Lets tests
/// substitute a mock backend, since the real `keyring` crate has no
/// built-in mock support and requires a real OS keyring session.
trait KeyringBackend {
    fn get_password(&self) -> Result<String, keyring::Error>;
    fn set_password(&self, token: &str) -> Result<(), keyring::Error>;
    fn delete_credential(&self) -> Result<(), keyring::Error>;
}

impl KeyringBackend for Entry {
    fn get_password(&self) -> Result<String, keyring::Error> {
        Entry::get_password(self)
    }

    fn set_password(&self, token: &str) -> Result<(), keyring::Error> {
        Entry::set_password(self, token)
    }

    fn delete_credential(&self) -> Result<(), keyring::Error> {
        Entry::delete_credential(self)
    }
}

/// Credential store backed by the OS keyring (Secret Service on Linux,
/// Keychain on macOS, Credential Manager on Windows), via the `keyring` crate.
pub struct KeyringCredentialStore {
    entry: Box<dyn KeyringBackend>,
}

impl KeyringCredentialStore {
    /// Creates a new keyring-backed store, failing if no usable keyring
    /// backend is available (e.g. no D-Bus session, locked keyring).
    pub fn new() -> Result<Self, CredentialStoreError> {
        let entry = Entry::new(SERVICE_NAME, CLI_USERNAME)
            .map_err(|e| CredentialStoreError::InitStore(e.to_string()))?;

        Ok(Self {
            entry: Box::new(entry),
        })
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn get_name(&self) -> &str {
        "keyring"
    }

    fn read_token(&self) -> Result<Option<String>, CredentialStoreError> {
        match self.entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(CredentialStoreError::TokenRead(e.to_string())),
        }
    }

    fn write_token(&self, token: &str) -> Result<(), CredentialStoreError> {
        self.entry
            .set_password(token)
            .map_err(|e| CredentialStoreError::TokenWrite(e.to_string()))
    }

    fn delete_token(&self) -> Result<(), CredentialStoreError> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CredentialStoreError::TokenDelete(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock keyring backend, taking inspiration from kfactory's
    /// `MockKeystore` (tests/unit/kfactory/credential_store/test_keyring_credential_store.py):
    /// an in-memory cache standing in for the real OS keyring, with the
    /// ability to force set/delete failures to exercise error handling.
    #[derive(Default)]
    struct MockKeyringBackend {
        cache: RefCell<Option<String>>,
        fail_set: bool,
        fail_delete: bool,
    }

    impl KeyringBackend for MockKeyringBackend {
        fn get_password(&self) -> Result<String, keyring::Error> {
            self.cache.borrow().clone().ok_or(keyring::Error::NoEntry)
        }

        fn set_password(&self, token: &str) -> Result<(), keyring::Error> {
            if self.fail_set {
                return Err(keyring::Error::Invalid(
                    "password".to_string(),
                    "mocked set failure".to_string(),
                ));
            }
            *self.cache.borrow_mut() = Some(token.to_string());
            Ok(())
        }

        fn delete_credential(&self) -> Result<(), keyring::Error> {
            if self.fail_delete {
                return Err(keyring::Error::Invalid(
                    "password".to_string(),
                    "mocked delete failure".to_string(),
                ));
            }
            *self.cache.borrow_mut() = None;
            Ok(())
        }
    }

    fn store_with(backend: MockKeyringBackend) -> KeyringCredentialStore {
        KeyringCredentialStore {
            entry: Box::new(backend),
        }
    }

    #[test]
    fn test_get_name() {
        let store = store_with(MockKeyringBackend::default());
        assert_eq!(store.get_name(), "keyring");
    }

    #[test]
    fn test_read_token_none_when_nothing_stored() {
        let store = store_with(MockKeyringBackend::default());
        assert_eq!(store.read_token().expect("read_token failed"), None);
    }

    #[test]
    fn test_write_then_read_token() {
        let store = store_with(MockKeyringBackend::default());

        store.write_token("secret").expect("write_token failed");

        assert_eq!(
            store.read_token().expect("read_token failed"),
            Some("secret".to_string())
        );
    }

    #[test]
    fn test_write_token_propagates_backend_error() {
        let store = store_with(MockKeyringBackend {
            fail_set: true,
            ..Default::default()
        });

        let err = store
            .write_token("secret")
            .expect_err("expected write_token to fail");
        assert!(matches!(err, CredentialStoreError::TokenWrite(_)));
    }

    #[test]
    fn test_delete_token_clears_stored_value() {
        let store = store_with(MockKeyringBackend::default());
        store.write_token("secret").expect("write_token failed");

        store.delete_token().expect("delete_token failed");

        assert_eq!(store.read_token().expect("read_token failed"), None);
    }

    #[test]
    fn test_delete_token_propagates_backend_error() {
        let store = store_with(MockKeyringBackend {
            fail_delete: true,
            ..Default::default()
        });

        let err = store
            .delete_token()
            .expect_err("expected delete_token to fail");
        assert!(matches!(err, CredentialStoreError::TokenDelete(_)));
    }
}
