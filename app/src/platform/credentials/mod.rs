//! Credential storage: OS-backed store plus an in-memory fake for tests.

pub mod keyring_store;
pub mod store;

pub use keyring_store::KeyringStore;
pub use store::{MemoryStore, SecretStore, keys};
