//! Secret storage abstraction.
//!
//! The application never falls back to plaintext storage. If the OS-backed
//! store is unavailable, operations fail closed and the UI reports the
//! failure.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use crate::domain::errors::CredentialStoreError;
use crate::domain::models::SecretString;

/// Service/account identifiers used with the credential store.
pub mod keys {
    pub const SERVICE: &str = "TikTokStreamTool";
    pub const STREAMLABS_TOKEN: &str = "streamlabs-token";
    pub const OBS_PASSWORD: &str = "obs-password";
}

/// An OS-backed (or test) secret store.
pub trait SecretStore: Send + Sync {
    /// Human-readable backend name for diagnostics (never secret).
    fn backend_name(&self) -> &'static str;

    fn set(
        &self,
        service: &str,
        account: &str,
        secret: &SecretString,
    ) -> Result<(), CredentialStoreError>;

    /// Returns `Ok(None)` when there is no stored secret (not an error).
    fn get(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialStoreError>;

    /// Delete a secret. Deleting a missing secret is not an error.
    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialStoreError>;
}

/// In-memory fake for development and tests. Never used as a silent fallback
/// in production; it is only constructed explicitly (e.g. under `cfg(test)`
/// or when the app is launched with a dev flag).
#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<HashMap<(String, String), String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.inner.lock().map(|m| m.len()).unwrap_or(0);
        write!(f, "MemoryStore({count} entries, values redacted)")
    }
}

impl SecretStore for MemoryStore {
    fn backend_name(&self) -> &'static str {
        "in-memory"
    }

    fn set(
        &self,
        service: &str,
        account: &str,
        secret: &SecretString,
    ) -> Result<(), CredentialStoreError> {
        let mut map = self
            .inner
            .lock()
            .map_err(|_| CredentialStoreError::Other("store poisoned".into()))?;
        map.insert(
            (service.to_string(), account.to_string()),
            secret.expose().to_string(),
        );
        Ok(())
    }

    fn get(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialStoreError> {
        let map = self
            .inner
            .lock()
            .map_err(|_| CredentialStoreError::Other("store poisoned".into()))?;
        Ok(map
            .get(&(service.to_string(), account.to_string()))
            .map(|v| SecretString::new(v.clone())))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CredentialStoreError> {
        let mut map = self
            .inner
            .lock()
            .map_err(|_| CredentialStoreError::Other("store poisoned".into()))?;
        map.remove(&(service.to_string(), account.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_delete_round_trip() {
        let store = MemoryStore::new();
        assert!(
            store
                .get(keys::SERVICE, keys::STREAMLABS_TOKEN)
                .unwrap()
                .is_none()
        );

        store
            .set(
                keys::SERVICE,
                keys::STREAMLABS_TOKEN,
                &SecretString::new("tok"),
            )
            .unwrap();
        let got = store.get(keys::SERVICE, keys::STREAMLABS_TOKEN).unwrap();
        assert_eq!(got.unwrap().expose(), "tok");

        store.delete(keys::SERVICE, keys::STREAMLABS_TOKEN).unwrap();
        assert!(
            store
                .get(keys::SERVICE, keys::STREAMLABS_TOKEN)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn deleting_missing_secret_is_ok() {
        let store = MemoryStore::new();
        assert!(store.delete(keys::SERVICE, "nope").is_ok());
    }

    #[test]
    fn memory_store_debug_does_not_leak_values() {
        let store = MemoryStore::new();
        store
            .set(
                keys::SERVICE,
                "acct",
                &SecretString::new("super-secret-value"),
            )
            .unwrap();
        let rendered = format!("{store:?}");
        assert!(!rendered.contains("super-secret-value"));
    }
}
