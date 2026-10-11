use open_parks::database::{Database, DatabaseError};
use open_parks::park_status::AlertCode;
use tempfile::TempDir;

#[tokio::test]
async fn a_fresh_database_has_no_last_code() {
    let directory = TempDir::new().unwrap();

    let database = Database::open(&directory.path().join("open-parks.sqlite3")).unwrap();

    assert_eq!(database.last_code().await.unwrap(), None);
}

#[tokio::test]
async fn the_last_code_saved_is_read_back_after_reopening() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    Database::open(&path)
        .unwrap()
        .save_last_code(AlertCode::Closed)
        .await
        .unwrap();

    let database = Database::open(&path).unwrap();

    assert_eq!(database.last_code().await.unwrap(), Some(AlertCode::Closed));
}

#[tokio::test]
async fn saving_replaces_the_last_code() {
    let directory = TempDir::new().unwrap();
    let database = Database::open(&directory.path().join("open-parks.sqlite3")).unwrap();

    database
        .save_last_code(AlertCode::RedAlertClosure)
        .await
        .unwrap();
    database.save_last_code(AlertCode::Open).await.unwrap();

    assert_eq!(database.last_code().await.unwrap(), Some(AlertCode::Open));
}

#[tokio::test]
async fn every_code_round_trips() {
    let directory = TempDir::new().unwrap();
    let database = Database::open(&directory.path().join("open-parks.sqlite3")).unwrap();
    let codes = [
        AlertCode::Open,
        AlertCode::Incidents,
        AlertCode::YellowAlert,
        AlertCode::OrangeAlert,
        AlertCode::RedAlertClosure,
        AlertCode::RedAlertRestrictions,
        AlertCode::Closed,
        AlertCode::Unknown(42),
    ];

    for code in codes {
        database.save_last_code(code).await.unwrap();
        assert_eq!(database.last_code().await.unwrap(), Some(code));
    }
}

#[test]
fn opening_fails_when_the_file_cannot_be_created() {
    let directory = TempDir::new().unwrap();

    let opened = Database::open(&directory.path().join("missing/open-parks.sqlite3"));

    assert!(opened.is_err());
}

#[test]
fn opening_fails_when_the_file_is_not_a_database() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    std::fs::write(&path, "not a database, just some text that is long enough").unwrap();

    assert!(Database::open(&path).is_err());
}

#[test]
fn opening_fails_when_the_schema_is_newer_than_this_build() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "user_version", 1000)
        .unwrap();
    drop(connection);

    assert!(matches!(
        Database::open(&path),
        Err(DatabaseError::NewerSchema(1000))
    ));
}
