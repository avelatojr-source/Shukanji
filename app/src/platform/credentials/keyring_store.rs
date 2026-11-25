//! Windows Credential Manager (and macOS Keychain / Linux Secret Service)
//! backed secret store using the `keyring` crate's v1 API.

use crate::domain::errors::CredentialStoreError;
use crate::domain::models::SecretString;

use super::store::SecretStore;

/// OS-backed secret store.
#[derive(Debug, Default, Clone)]
pub struct KeyringStore;

fn map_err(err: keyring::Error) -> CredentialStoreError {
    match err {
        keyring::Error::NoEntry => CredentialStoreError::NotFound,
        keyring::Error::NoDefaultStore => {
            CredentialStoreError::Unavailable("no default credential store available".into())
        }
        keyring::Error::PlatformFailure(_) => CredentialStoreError::Unavailable(err.to_string()),
        keyring::Error::NoStorageAccess(_) => CredentialStoreError::Unavailable(err.to_string()),
        other => CredentialStoreError::Other(other.to_string()),
    }
}

impl KeyringStore {
    fn entry(service: &str, account: &str) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(service, account).map_err(map_err)
    }
}

impl SecretStore for KeyringStore {
    fn backend_name(&self) -> &'static str {
        "os-credential-manager"
    }

    fn set(
        &self,
        service: &str,
        account: &str,
        secret: &SecretString,
    ) -> Result<(), CredentialStoreError> {
        let entry = Self::entry(service, account)?;
        entry.set_password(secret.expose()).map_err(map_err)
    }

    fn get(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialStoreError> {
        let entry = Self::entry(service, account)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(map_err(other)),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialStoreError> {
        let entry = Self::entry(service, account)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            // Deleting a missing credential is not an error.
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(other) => Err(map_err(other)),
        }
    }
}
