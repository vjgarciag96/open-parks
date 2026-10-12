use std::path::Path;
use std::time::Duration;

use axum::http::StatusCode;
use open_parks::database::Database;
use open_parks::notifier::{self, Channels, LastStatus, Notification, notification};
use open_parks::park_status::{AlertCode, ParkStatus, ReopeningForecast, Verdict};
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma::{self, FetchError};
use tempfile::TempDir;
use tokio::sync::watch;

mod common;
mod timeouts;
use common::fixture;
use timeouts::GENEROUS_TIMEOUT;

const OFTEN: Duration = Duration::from_millis(10);

const OPEN_CODES: [AlertCode; 5] = [
    AlertCode::Open,
    AlertCode::Incidents,
    AlertCode::YellowAlert,
    AlertCode::OrangeAlert,
    AlertCode::RedAlertRestrictions,
];

const CLOSING_CODES: [AlertCode; 2] = [AlertCode::RedAlertClosure, AlertCode::Closed];

fn decoded(fixture_name: &str) -> ParkStatus {
    sigma::decode(&fixture(fixture_name)).unwrap()
}

fn fetched(fixture_name: &str) -> Latest {
    Ok(Fetched {
        status: decoded(fixture_name),
        fetched_at: "2026-10-11T09:30:00Z".parse().unwrap(),
    })
}

fn failed() -> Latest {
    Err(FetchError::HttpStatus(StatusCode::SERVICE_UNAVAILABLE))
}

fn last_status(path: &Path) -> LastStatus {
    LastStatus::new(Database::open(path).unwrap())
}

async fn stored(path: &Path) -> Option<AlertCode> {
    last_status(path).code().await.unwrap()
}

async fn store(path: &Path, code: AlertCode) {
    last_status(path).save(code).await.unwrap();
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
    notifier::start(last_status(path), latest, Channels::new())
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

    assert!(
        notifier::start(LastStatus::new(database), receiver, Channels::new())
            .await
            .is_err()
    );
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

#[tokio::test]
async fn closed_then_unknown_then_closed_sends_nothing() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let (latest, receiver) = watch::channel(fetched("code_7_closed.json"));
    start(&path, receiver).await;
    wait_until_stored(&path, AlertCode::Closed).await;
    latest.send(fetched("unknown_codes.json")).unwrap();
    tokio::time::sleep(OFTEN * 5).await;

    let sent = notification(stored(&path).await, &decoded("code_7_closed.json"));

    assert_eq!(sent, None);
}

#[tokio::test]
async fn closed_then_unknown_then_open_sends_one_reopening() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let (latest, receiver) = watch::channel(fetched("code_7_closed.json"));
    start(&path, receiver).await;
    wait_until_stored(&path, AlertCode::Closed).await;
    latest.send(fetched("unknown_codes.json")).unwrap();
    tokio::time::sleep(OFTEN * 5).await;
    let open = decoded("code_1_open.json");

    let sent = notification(stored(&path).await, &open);

    assert_eq!(
        sent,
        Some(Notification {
            old: Verdict::Closed,
            new: Verdict::Open,
            status: open,
        })
    );
}

fn status(code: AlertCode) -> ParkStatus {
    ParkStatus {
        code,
        dates: None,
        hours: None,
        reopening: None,
        notes: None,
    }
}

fn notifies(baseline: AlertCode, code: AlertCode) -> bool {
    notification(Some(baseline), &status(code)).is_some()
}

#[test]
fn no_baseline_does_not_notify() {
    for code in OPEN_CODES.into_iter().chain(CLOSING_CODES) {
        assert_eq!(notification(None, &status(code)), None, "{code:?}");
    }
}

#[test]
fn a_forecast_closure_notifies_from_any_open_verdict() {
    for baseline in OPEN_CODES {
        assert!(
            notifies(baseline, AlertCode::RedAlertClosure),
            "{baseline:?}"
        );
    }
}

#[test]
fn a_closure_notifies_from_any_open_verdict() {
    for baseline in OPEN_CODES {
        assert!(notifies(baseline, AlertCode::Closed), "{baseline:?}");
    }
}

#[test]
fn a_forecast_closure_escalating_to_closed_notifies() {
    let sent = notification(Some(AlertCode::RedAlertClosure), &status(AlertCode::Closed));

    assert_eq!(
        sent,
        Some(Notification {
            old: Verdict::ClosureForecast,
            new: Verdict::Closed,
            status: status(AlertCode::Closed),
        })
    );
}

#[test]
fn a_closed_park_downgrading_to_a_forecast_closure_notifies() {
    let sent = notification(Some(AlertCode::Closed), &status(AlertCode::RedAlertClosure));

    assert_eq!(
        sent,
        Some(Notification {
            old: Verdict::Closed,
            new: Verdict::ClosureForecast,
            status: status(AlertCode::RedAlertClosure),
        })
    );
}

#[test]
fn a_reopening_to_any_open_verdict_notifies() {
    for baseline in CLOSING_CODES {
        for code in OPEN_CODES {
            let sent = notification(Some(baseline), &status(code));

            assert_eq!(
                sent,
                Some(Notification {
                    old: baseline.verdict(),
                    new: code.verdict(),
                    status: status(code),
                }),
                "{baseline:?} → {code:?}"
            );
        }
    }
}

#[test]
fn changes_between_open_verdicts_do_not_notify() {
    for baseline in OPEN_CODES {
        for code in OPEN_CODES {
            assert!(!notifies(baseline, code), "{baseline:?} → {code:?}");
        }
    }
}

#[test]
fn a_change_in_the_details_alone_does_not_notify() {
    for code in CLOSING_CODES {
        let changed = ParkStatus {
            dates: Some("Del 12 al 14 de octubre".into()),
            hours: Some("Todo el día".into()),
            reopening: Some(ReopeningForecast::AfterInspections),
            notes: Some("Rachas de viento".into()),
            ..status(code)
        };

        assert_eq!(notification(Some(code), &changed), None, "{code:?}");
    }
}

#[test]
fn an_unknown_code_does_not_notify() {
    for baseline in OPEN_CODES.into_iter().chain(CLOSING_CODES) {
        assert!(!notifies(baseline, AlertCode::Unknown(8)), "{baseline:?}");
    }
}

#[test]
fn an_unknown_stored_code_does_not_notify() {
    for code in OPEN_CODES.into_iter().chain(CLOSING_CODES) {
        assert!(!notifies(AlertCode::Unknown(8), code), "{code:?}");
    }
}

#[test]
fn the_notification_carries_both_verdicts_and_the_new_status() {
    let closed = ParkStatus {
        dates: Some("12/10/2026".into()),
        hours: Some("Todo el día".into()),
        reopening: Some(ReopeningForecast::SeeNotes),
        notes: Some("Caída de ramas".into()),
        ..status(AlertCode::Closed)
    };

    let sent = notification(Some(AlertCode::OrangeAlert), &closed);

    assert_eq!(
        sent,
        Some(Notification {
            old: Verdict::OpenStayNotAdvised,
            new: Verdict::Closed,
            status: closed,
        })
    );
}
