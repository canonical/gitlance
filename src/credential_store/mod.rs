// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

//! Secure storage for the OpenRouter API key.
//!
//! Mirrors the credential store design used by `kfactory`
//! (`kfactory.credential_store`): a `CredentialStore` trait with a
//! keyring-backed implementation as the primary store and a plain-file
//! implementation as a fallback for environments without a usable keyring
//! (e.g. headless CI without a D-Bus session).

pub mod error;
pub mod factory;
pub mod file_store;
pub mod keyring_store;

pub use error::CredentialStoreError;
pub use factory::CredentialStoreFactory;
pub use file_store::FileCredentialStore;
pub use keyring_store::KeyringCredentialStore;

/// A store capable of persisting a single secret token (the OpenRouter API key).
pub trait CredentialStore {
    /// Returns a human-readable name for this store (e.g. "keyring", "file").
    fn get_name(&self) -> &str;

    /// Reads the stored token, if any.
    fn read_token(&self) -> Result<Option<String>, CredentialStoreError>;

    /// Stores a token, overwriting any existing value.
    fn write_token(&self, token: &str) -> Result<(), CredentialStoreError>;

    /// Deletes the stored token, if any.
    fn delete_token(&self) -> Result<(), CredentialStoreError>;
}
