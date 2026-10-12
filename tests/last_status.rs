use open_parks::database::Database;
use open_parks::notifier::LastStatus;
use open_parks::park_status::AlertCode;
use tempfile::TempDir;

fn in_a_fresh_database(directory: &TempDir) -> LastStatus {
    LastStatus::new(Database::open(&directory.path().join("open-parks.sqlite3")).unwrap())
}

#[tokio::test]
async fn a_fresh_database_has_no_last_status() {
    let directory = TempDir::new().unwrap();

    let last_status = in_a_fresh_database(&directory);

    assert_eq!(last_status.code().await.unwrap(), None);
}

#[tokio::test]
async fn the_last_code_saved_is_read_back_after_reopening() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    LastStatus::new(Database::open(&path).unwrap())
        .save(AlertCode::Closed)
        .await
        .unwrap();

    let last_status = LastStatus::new(Database::open(&path).unwrap());

    assert_eq!(last_status.code().await.unwrap(), Some(AlertCode::Closed));
}

#[tokio::test]
async fn saving_replaces_the_last_code() {
    let directory = TempDir::new().unwrap();
    let last_status = in_a_fresh_database(&directory);

    last_status.save(AlertCode::RedAlertClosure).await.unwrap();
    last_status.save(AlertCode::Open).await.unwrap();

    assert_eq!(last_status.code().await.unwrap(), Some(AlertCode::Open));
}

#[tokio::test]
async fn every_code_round_trips() {
    let directory = TempDir::new().unwrap();
    let last_status = in_a_fresh_database(&directory);
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
        last_status.save(code).await.unwrap();
        assert_eq!(last_status.code().await.unwrap(), Some(code));
    }
}
