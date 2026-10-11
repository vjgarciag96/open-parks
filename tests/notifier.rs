use std::path::Path;
use std::time::Duration;

use axum::http::StatusCode;
use open_parks::database::Database;
use open_parks::notifier;
use open_parks::park_status::AlertCode;
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma::{self, FetchError};
use tempfile::TempDir;
use tokio::sync::watch;

mod common;
mod timeouts;
use common::fixture;
use timeouts::GENEROUS_TIMEOUT;

const OFTEN: Duration = Duration::from_millis(10);

fn fetched(fixture_name: &str) -> Latest {
    Ok(Fetched {
        status: sigma::decode(&fixture(fixture_name)).unwrap(),
        fetched_at: "2026-10-11T09:30:00Z".parse().unwrap(),
    })
}

fn failed() -> Latest {
    Err(FetchError::HttpStatus(StatusCode::SERVICE_UNAVAILABLE))
}

async fn stored(path: &Path) -> Option<AlertCode> {
    Database::open(path).unwrap().last_code().await.unwrap()
}

async fn store(path: &Path, code: AlertCode) {
    Database::open(path)
        .unwrap()
        .save_last_code(code)
        .await
        .unwrap();
}

async fn wait_until_stored(path: &Path, expected: AlertCode) {
    let saved = async {
        while stored(path).await != Some(expected) {
            tokio::time::sleep(OFTEN).await;
        }
    };
    if tokio::time::timeout(GENEROUS_TIMEOUT, saved).await.is_err() {
        panic!(
            "{expected:?} was never stored; found {:?}",
            stored(path).await
        );
    }
}

async fn start(path: &Path, latest: watch::Receiver<Latest>) {
    notifier::start(Database::open(path).unwrap(), latest)
        .await
        .unwrap();
}

#[tokio::test]
async fn the_status_at_startup_is_stored() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let (_latest, receiver) = watch::channel(fetched("code_7_closed.json"));

    start(&path, receiver).await;

    wait_until_stored(&path, AlertCode::Closed).await;
}

#[tokio::test]
async fn a_changed_status_is_stored() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let (latest, receiver) = watch::channel(fetched("code_1_open.json"));
    start(&path, receiver).await;
    wait_until_stored(&path, AlertCode::Open).await;

    latest
        .send(fetched("code_5_red_alert_closure.json"))
        .unwrap();

    wait_until_stored(&path, AlertCode::RedAlertClosure).await;
}

#[tokio::test]
async fn a_status_stored_before_a_restart_is_replaced_by_the_one_fetched() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    store(&path, AlertCode::Closed).await;
    let (_latest, receiver) = watch::channel(fetched("code_1_open.json"));

    start(&path, receiver).await;

    wait_until_stored(&path, AlertCode::Open).await;
}

#[tokio::test]
async fn a_failed_fetch_keeps_the_stored_code() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    store(&path, AlertCode::Closed).await;
    let (latest, receiver) = watch::channel(failed());
    start(&path, receiver).await;

    latest.send(failed()).unwrap();
    tokio::time::sleep(OFTEN * 5).await;

    assert_eq!(stored(&path).await, Some(AlertCode::Closed));
}

#[tokio::test]
async fn startup_fails_when_the_stored_code_cannot_be_read() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let database = Database::open(&path).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("INSERT INTO last_status (id, code) VALUES (1, 100000)", [])
        .unwrap();
    let (_latest, receiver) = watch::channel(fetched("code_7_closed.json"));

    assert!(notifier::start(database, receiver).await.is_err());
}

#[tokio::test]
async fn an_unknown_code_keeps_the_stored_code() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let (latest, receiver) = watch::channel(fetched("code_7_closed.json"));
    start(&path, receiver).await;
    wait_until_stored(&path, AlertCode::Closed).await;

    latest.send(fetched("unknown_codes.json")).unwrap();
    tokio::time::sleep(OFTEN * 5).await;

    assert_eq!(stored(&path).await, Some(AlertCode::Closed));
}
