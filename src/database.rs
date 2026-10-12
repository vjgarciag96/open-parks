use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::park_status::AlertCode;
use crate::sigma;

// Append only; never edit a migration that has shipped.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE last_status (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        code INTEGER NOT NULL
    ) STRICT;",
    "CREATE TABLE telegram_subscribers (
        chat_id INTEGER PRIMARY KEY,
        language TEXT NOT NULL CHECK (language IN ('es', 'en'))
    ) STRICT;",
];

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("the schema is at version {0}, newer than this build's {latest}", latest = MIGRATIONS.len())]
    NewerSchema(i64),
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub async fn last_code(&self) -> Result<Option<AlertCode>, DatabaseError> {
        self.query(|connection| {
            let code = connection
                .query_row("SELECT code FROM last_status", [], |row| row.get(0))
                .optional()?;
            Ok(code.map(sigma::alert_code))
        })
        .await
    }

    pub async fn save_last_code(&self, code: AlertCode) -> Result<(), DatabaseError> {
        self.query(move |connection| {
            connection.execute(
                "INSERT INTO last_status (id, code) VALUES (1, ?1)
                 ON CONFLICT (id) DO UPDATE SET code = excluded.code",
                [sigma::raw_alert_code(code)],
            )?;
            Ok(())
        })
        .await
    }

    // SQLite blocks, so queries run off the async workers.
    pub(crate) async fn query<T: Send + 'static>(
        &self,
        query: impl FnOnce(&Connection) -> Result<T, DatabaseError> + Send + 'static,
    ) -> Result<T, DatabaseError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || query(&connection.lock().unwrap()))
            .await
            .unwrap_or_else(|error| std::panic::resume_unwind(error.into_panic()))
    }
}

fn migrate(connection: &mut Connection) -> Result<(), DatabaseError> {
    // Locks before reading the version, so two connections can't both apply
    // the same migration.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version = schema_version(&transaction)?;
    let applied = usize::try_from(version)
        .ok()
        .filter(|&applied| applied <= MIGRATIONS.len())
        .ok_or(DatabaseError::NewerSchema(version))?;
    if applied == MIGRATIONS.len() {
        return Ok(());
    }
    for migration in &MIGRATIONS[applied..] {
        transaction.execute_batch(migration)?;
    }
    transaction.pragma_update(None, "user_version", MIGRATIONS.len() as i64)?;
    transaction.commit()?;
    Ok(())
}

fn schema_version(transaction: &Transaction) -> rusqlite::Result<i64> {
    transaction.pragma_query_value(None, "user_version", |row| row.get(0))
}
