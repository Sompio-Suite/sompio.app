//! Live connections to people's own accounts.
//!
//! This is the only part of Sompio that talks to the network, and it is kept apart from
//! sompio-clean on purpose: the engine stays offline, and a connector only runs after the
//! person connects an account themselves. Each connector talks only to its own platform.
//!
//! Secrets (login sessions, app keys) live in the operating system's credential store
//! (Windows Credential Manager), never in Sompio's database or in plain files.

pub mod email;
pub mod telegram;

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Mutex;

/// Where connectors keep secrets. The real one is the OS credential store; tests use memory.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Windows Credential Manager (or the macOS Keychain / Secret Service elsewhere).
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: &str) -> Self {
        Self { service: service.to_string() }
    }
    fn entry(&self, key: &str) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, key)?)
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        Ok(self.entry(key)?.set_password(value)?)
    }
    fn delete(&self, key: &str) -> Result<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Keeps secrets in memory only. For tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(key.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}
