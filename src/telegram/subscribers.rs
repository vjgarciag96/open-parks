use rusqlite::OptionalExtension;

use super::ChatId;
use crate::database::{Database, DatabaseError};
use crate::strings::Lang;

pub struct Subscribers {
    database: Database,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscriber {
    pub chat: ChatId,
    pub lang: Lang,
}

impl Subscribers {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn add(&self, subscriber: Subscriber) -> Result<(), DatabaseError> {
        self.database
            .query(move |connection| {
                connection.execute(
                    "INSERT INTO telegram_subscribers (chat_id, language) VALUES (?1, ?2)
                     ON CONFLICT (chat_id) DO UPDATE SET language = excluded.language",
                    (subscriber.chat.0, subscriber.lang.strings().code),
                )?;
                Ok(())
            })
            .await
    }

    pub async fn remove(&self, chat: ChatId) -> Result<(), DatabaseError> {
        self.database
            .query(move |connection| {
                connection.execute(
                    "DELETE FROM telegram_subscribers WHERE chat_id = ?1",
                    [chat.0],
                )?;
                Ok(())
            })
            .await
    }

    pub async fn lang(&self, chat: ChatId) -> Result<Option<Lang>, DatabaseError> {
        self.database
            .query(move |connection| {
                let code: Option<String> = connection
                    .query_row(
                        "SELECT language FROM telegram_subscribers WHERE chat_id = ?1",
                        [chat.0],
                        |row| row.get(0),
                    )
                    .optional()?;
                Ok(code.map(|code| stored_lang(&code)))
            })
            .await
    }

    pub async fn all(&self) -> Result<Vec<Subscriber>, DatabaseError> {
        self.database
            .query(|connection| {
                let mut statement = connection.prepare(
                    "SELECT chat_id, language FROM telegram_subscribers ORDER BY chat_id",
                )?;
                let subscribers = statement
                    .query_map([], |row| {
                        Ok(Subscriber {
                            chat: ChatId(row.get(0)?),
                            lang: stored_lang(&row.get::<_, String>(1)?),
                        })
                    })?
                    .collect::<Result<_, _>>()?;
                Ok(subscribers)
            })
            .await
    }
}

fn stored_lang(code: &str) -> Lang {
    // Only ever written from a Lang, so anything else is a bug.
    Lang::from_tag(code).unwrap_or_else(|| panic!("unknown stored language {code:?}"))
}
