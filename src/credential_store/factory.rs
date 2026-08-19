// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::credential_store::{
    CredentialStore, CredentialStoreError, FileCredentialStore, KeyringCredentialStore,
};

/// Creates the best available credential store: tries the OS keyring first,
/// falling back to a plain file under `$XDG_RUNTIME_DIR` if the keyring is
/// unavailable (e.g. locked, no D-Bus session), mirroring kfactory's
/// `CredentialStoreFactory.create_store()`.
pub struct CredentialStoreFactory;

impl CredentialStoreFactory {
    /// Returns an error only if neither the keyring nor the file fallback
    /// could be initialized (e.g. no keyring backend AND `XDG_RUNTIME_DIR` unset).
    pub fn create_store() -> Result<Box<dyn CredentialStore>, CredentialStoreError> {
        match KeyringCredentialStore::new() {
            Ok(store) => Ok(Box::new(store)),
            Err(keyring_err) => FileCredentialStore::new()
                .map(|store| Box::new(store) as Box<dyn CredentialStore>)
                .map_err(|file_err| {
                    CredentialStoreError::InitStore(format!(
                        "no usable credential store (keyring: {}; file: {})",
                        keyring_err, file_err
                    ))
                }),
        }
    }
}
