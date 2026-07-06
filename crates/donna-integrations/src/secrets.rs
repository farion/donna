use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Mutex;

#[derive(Debug)]
pub enum SecretError {
    Backend(keyring::Error),
    StorePoisoned,
}

pub trait SecretStore {
    fn get_secret(&self, reference: &str) -> Result<Option<String>, SecretError>;
    fn set_secret(&self, reference: &str, value: &str) -> Result<(), SecretError>;
    fn delete_secret(&self, reference: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, reference: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, reference).map_err(SecretError::Backend)
    }

    fn reference_aliases(reference: &str) -> Vec<String> {
        let mut aliases = vec![reference.to_owned()];
        if reference.contains('/') {
            aliases.push(reference.replace('/', ":"));
            if let Some(short) = reference.rsplit('/').next().filter(|part| !part.is_empty()) {
                aliases.push(short.to_owned());
            }
        }
        aliases.sort();
        aliases.dedup();
        aliases
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new("donna")
    }
}

impl SecretStore for KeyringSecretStore {
    fn get_secret(&self, reference: &str) -> Result<Option<String>, SecretError> {
        for alias in Self::reference_aliases(reference) {
            match self.entry(&alias)?.get_password() {
                Ok(secret) => return Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => {}
                Err(error) => return Err(SecretError::Backend(error)),
            }
        }
        Ok(None)
    }

    fn set_secret(&self, reference: &str, value: &str) -> Result<(), SecretError> {
        for alias in Self::reference_aliases(reference) {
            self.entry(&alias)?
                .set_password(value)
                .map_err(SecretError::Backend)?;
        }
        Ok(())
    }

    fn delete_secret(&self, reference: &str) -> Result<(), SecretError> {
        for alias in Self::reference_aliases(reference) {
            match self.entry(&alias)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(error) => return Err(SecretError::Backend(error)),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for InMemorySecretStore {
    fn get_secret(&self, reference: &str) -> Result<Option<String>, SecretError> {
        let values = self.values.lock().map_err(|_| SecretError::StorePoisoned)?;
        Ok(values.get(reference).cloned())
    }

    fn set_secret(&self, reference: &str, value: &str) -> Result<(), SecretError> {
        let mut values = self.values.lock().map_err(|_| SecretError::StorePoisoned)?;
        values.insert(reference.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete_secret(&self, reference: &str) -> Result<(), SecretError> {
        let mut values = self.values.lock().map_err(|_| SecretError::StorePoisoned)?;
        values.remove(reference);
        Ok(())
    }
}

impl Display for SecretError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretError::Backend(source) => write!(formatter, "secret store error: {source}"),
            SecretError::StorePoisoned => write!(formatter, "secret store lock was poisoned"),
        }
    }
}

impl Error for SecretError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SecretError::Backend(source) => Some(source),
            SecretError::StorePoisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemorySecretStore, KeyringSecretStore, SecretStore};

    #[test]
    fn missing_secret_returns_none() {
        let store = InMemorySecretStore::default();

        assert_eq!(store.get_secret("donna/openai").expect("read"), None);
    }

    #[test]
    fn in_memory_store_round_trips_without_config() {
        let store = InMemorySecretStore::default();

        store
            .set_secret("donna/openai", "super-secret")
            .expect("write");

        assert_eq!(
            store.get_secret("donna/openai").expect("read").as_deref(),
            Some("super-secret")
        );

        store.delete_secret("donna/openai").expect("delete");
        assert_eq!(store.get_secret("donna/openai").expect("read"), None);
    }

    #[test]
    fn keyring_references_with_slashes_have_safe_aliases() {
        assert_eq!(
            KeyringSecretStore::reference_aliases("donna/openai"),
            vec![
                "donna/openai".to_owned(),
                "donna:openai".to_owned(),
                "openai".to_owned(),
            ]
        );
    }
}
