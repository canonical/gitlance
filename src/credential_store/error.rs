// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

/// Errors that can occur while working with a credential store.
#[derive(Debug)]
pub enum CredentialStoreError {
    /// The store could not be initialized (e.g. no keyring backend available).
    InitStore(String),
    /// Failed to read the token from the store.
    TokenRead(String),
    /// Failed to write the token to the store.
    TokenWrite(String),
    /// Failed to delete the token from the store.
    TokenDelete(String),
}

impl fmt::Display for CredentialStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialStoreError::InitStore(msg) => {
                write!(f, "Credential store init error: {}", msg)
            }
            CredentialStoreError::TokenRead(msg) => write!(f, "Token read error: {}", msg),
            CredentialStoreError::TokenWrite(msg) => write!(f, "Token write error: {}", msg),
            CredentialStoreError::TokenDelete(msg) => write!(f, "Token delete error: {}", msg),
        }
    }
}

impl std::error::Error for CredentialStoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_store_display() {
        let err = CredentialStoreError::InitStore("no backend".to_string());
        assert_eq!(err.to_string(), "Credential store init error: no backend");
    }

    #[test]
    fn test_token_read_display() {
        let err = CredentialStoreError::TokenRead("missing".to_string());
        assert_eq!(err.to_string(), "Token read error: missing");
    }

    #[test]
    fn test_token_write_display() {
        let err = CredentialStoreError::TokenWrite("denied".to_string());
        assert_eq!(err.to_string(), "Token write error: denied");
    }

    #[test]
    fn test_token_delete_display() {
        let err = CredentialStoreError::TokenDelete("not present".to_string());
        assert_eq!(err.to_string(), "Token delete error: not present");
    }
}
