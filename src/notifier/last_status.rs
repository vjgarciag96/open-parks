use rusqlite::OptionalExtension;

use crate::database::{Database, DatabaseError};
use crate::park_status::AlertCode;
use crate::sigma;

#[derive(Clone)]
pub struct LastStatus {
    database: Database,
}

impl LastStatus {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn code(&self) -> Result<Option<AlertCode>, DatabaseError> {
        self.database
            .query(|connection| {
                let code = connection
                    .query_row("SELECT code FROM last_status", [], |row| row.get(0))
                    .optional()?;
                Ok(code.map(sigma::alert_code))
            })
            .await
    }

    pub async fn save(&self, code: AlertCode) -> Result<(), DatabaseError> {
        self.database
            .query(move |connection| {
                connection.execute(
                    "INSERT INTO last_status (id, code) VALUES (1, ?1)
                     ON CONFLICT (id) DO UPDATE SET code = excluded.code",
                    [sigma::raw_alert_code(code)],
                )?;
                Ok(())
            })
            .await
    }
}
