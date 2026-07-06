use crate::storage::migrations;
use rusqlite::Connection;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Clock(SystemTimeError),
}

pub struct LocalStore {
    pub(super) connection: Connection,
}

impl LocalStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }

        let connection = Connection::open(path).map_err(StorageError::Sqlite)?;
        Self::from_connection(connection)
    }

    pub fn in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory().map_err(StorageError::Sqlite)?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self, StorageError> {
        let store = Self { connection };
        store
            .connection
            .execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::apply_migrations(&store.connection)?;
        Ok(store)
    }

    pub fn applied_migration_versions(&self) -> Result<Vec<i64>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")?;

        let rows = statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }
}

pub(super) fn now_seconds() -> Result<i64, StorageError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(StorageError::Clock)?;
    Ok(elapsed.as_secs() as i64)
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        StorageError::Sqlite(error)
    }
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(source) => write!(formatter, "storage IO error: {source}"),
            StorageError::Sqlite(source) => write!(formatter, "sqlite storage error: {source}"),
            StorageError::Clock(source) => write!(formatter, "system clock error: {source}"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StorageError::Io(source) => Some(source),
            StorageError::Sqlite(source) => Some(source),
            StorageError::Clock(source) => Some(source),
        }
    }
}
