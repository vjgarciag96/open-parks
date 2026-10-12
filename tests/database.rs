use open_parks::database::{Database, DatabaseError};
use tempfile::TempDir;

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
