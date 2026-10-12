use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::Timestamp;
use open_parks::database::Database;
use open_parks::notifier::{self, Channel, Channels, Notification};
use open_parks::park_status::{AlertCode, ParkStatus, Verdict};
use open_parks::poller::{self, Latest};
use open_parks::sigma;
use reqwest::Url;
use tempfile::TempDir;
use tokio::sync::{Semaphore, mpsc, watch};

mod common;
mod fake_upstream;
mod timeouts;
use common::fixture;
use fake_upstream::{json, serving};
use timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};

const OFTEN: Duration = Duration::from_millis(10);

struct Upstream {
    url: Url,
    body: Arc<Mutex<String>>,
}

impl Upstream {
    async fn serving(fixture_name: &str) -> Self {
        let body = Arc::new(Mutex::new(fixture(fixture_name)));
        let url = serving({
            let body = Arc::clone(&body);
            || async move { json(body.lock().unwrap().clone()) }
        })
        .await;
        Self { url, body }
    }

    fn serve(&self, fixture_name: &str) {
        *self.body.lock().unwrap() = fixture(fixture_name);
    }
}

struct Recording(mpsc::UnboundedSender<Notification>);

impl Channel for Recording {
    async fn send(&mut self, notification: Notification) {
        let _ = self.0.send(notification);
    }
}

fn recording() -> (Recording, mpsc::UnboundedReceiver<Notification>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Recording(sender), receiver)
}

struct Held {
    released: Arc<Semaphore>,
    recording: Recording,
}

impl Channel for Held {
    async fn send(&mut self, notification: Notification) {
        self.recording.send(notification).await;
        self.released.acquire().await.unwrap().forget();
    }
}

fn decoded(fixture_name: &str) -> ParkStatus {
    sigma::decode(&fixture(fixture_name)).unwrap()
}

fn fetch_time() -> Timestamp {
    "2026-10-11T09:30:00Z".parse().unwrap()
}

async fn poll(upstream: &Upstream) -> watch::Receiver<Latest> {
    let client = sigma::Client::new(upstream.url.clone(), GENEROUS_TIMEOUT).unwrap();
    poller::start(client, OFTEN, fetch_time).await
}

async fn start(path: &Path, latest: watch::Receiver<Latest>, channels: Channels) {
    notifier::start(Database::open(path).unwrap(), latest, channels)
        .await
        .unwrap();
}

async fn next(received: &mut mpsc::UnboundedReceiver<Notification>) -> Notification {
    tokio::time::timeout(GENEROUS_TIMEOUT, received.recv())
        .await
        .expect("no notification arrived")
        .unwrap()
}

async fn assert_nothing_more(received: &mut mpsc::UnboundedReceiver<Notification>) {
    tokio::time::sleep(SHORT_TIMEOUT).await;
    assert_eq!(received.try_recv().ok(), None);
}

async fn wait_for_code(latest: &mut watch::Receiver<Latest>, code: AlertCode) {
    let published = latest.wait_for(|latest| {
        latest
            .as_ref()
            .is_ok_and(|fetched| fetched.status.code == code)
    });
    tokio::time::timeout(GENEROUS_TIMEOUT, published)
        .await
        .unwrap_or_else(|_| panic!("{code:?} was never published"))
        .unwrap();
}

async fn stored(path: &Path) -> Option<AlertCode> {
    Database::open(path).unwrap().last_code().await.unwrap()
}

async fn wait_until_stored(path: &Path, expected: AlertCode) {
    let saved = async {
        while stored(path).await != Some(expected) {
            tokio::time::sleep(OFTEN).await;
        }
    };
    tokio::time::timeout(GENEROUS_TIMEOUT, saved)
        .await
        .unwrap_or_else(|_| panic!("{expected:?} was never stored"));
}

fn notification(old: Verdict, new: Verdict, fixture_name: &str) -> Notification {
    Notification {
        old,
        new,
        status: decoded(fixture_name),
    }
}

#[tokio::test]
async fn a_closure_and_the_reopening_reach_the_channel_in_order() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let upstream = Upstream::serving("code_1_open.json").await;
    let (channel, mut received) = recording();
    start(&path, poll(&upstream).await, Channels::new().with(channel)).await;
    wait_until_stored(&path, AlertCode::Open).await;

    upstream.serve("code_7_closed.json");
    wait_until_stored(&path, AlertCode::Closed).await;
    upstream.serve("code_1_open.json");

    assert_eq!(
        next(&mut received).await,
        notification(Verdict::Open, Verdict::Closed, "code_7_closed.json")
    );
    assert_eq!(
        next(&mut received).await,
        notification(Verdict::Closed, Verdict::Open, "code_1_open.json")
    );
    assert_nothing_more(&mut received).await;
}

#[tokio::test]
async fn every_channel_receives_the_notification() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let upstream = Upstream::serving("code_1_open.json").await;
    let (first, mut first_received) = recording();
    let (second, mut second_received) = recording();
    let channels = Channels::new().with(first).with(second);
    start(&path, poll(&upstream).await, channels).await;
    wait_until_stored(&path, AlertCode::Open).await;

    upstream.serve("code_5_red_alert_closure.json");

    let expected = notification(
        Verdict::Open,
        Verdict::ClosureForecast,
        "code_5_red_alert_closure.json",
    );
    assert_eq!(next(&mut first_received).await, expected);
    assert_eq!(next(&mut second_received).await, expected);
}

#[tokio::test]
async fn a_fresh_database_takes_the_first_fetch_as_the_baseline() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let upstream = Upstream::serving("code_7_closed.json").await;
    let (channel, mut received) = recording();

    start(&path, poll(&upstream).await, Channels::new().with(channel)).await;

    wait_until_stored(&path, AlertCode::Closed).await;
    assert_nothing_more(&mut received).await;
}

#[tokio::test]
async fn after_a_restart_the_first_fetch_is_compared_to_the_stored_status() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    Database::open(&path)
        .unwrap()
        .save_last_code(AlertCode::Closed)
        .await
        .unwrap();
    let upstream = Upstream::serving("code_1_open.json").await;
    let (channel, mut received) = recording();

    start(&path, poll(&upstream).await, Channels::new().with(channel)).await;

    assert_eq!(
        next(&mut received).await,
        notification(Verdict::Closed, Verdict::Open, "code_1_open.json")
    );
    assert_nothing_more(&mut received).await;
}

#[tokio::test]
async fn an_unknown_code_does_not_move_the_baseline() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let upstream = Upstream::serving("code_7_closed.json").await;
    let (channel, mut received) = recording();
    let mut latest = poll(&upstream).await;
    start(&path, latest.clone(), Channels::new().with(channel)).await;
    wait_until_stored(&path, AlertCode::Closed).await;

    upstream.serve("unknown_codes.json");
    wait_for_code(&mut latest, AlertCode::Unknown(8)).await;
    upstream.serve("code_1_open.json");

    assert_eq!(
        next(&mut received).await,
        notification(Verdict::Closed, Verdict::Open, "code_1_open.json")
    );
    assert_nothing_more(&mut received).await;
}

#[tokio::test]
async fn a_slow_channel_holds_back_neither_the_others_nor_the_notifier() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("open-parks.sqlite3");
    let upstream = Upstream::serving("code_1_open.json").await;
    let released = Arc::new(Semaphore::new(0));
    let (held_recording, mut held_received) = recording();
    let held = Held {
        released: Arc::clone(&released),
        recording: held_recording,
    };
    let (channel, mut received) = recording();
    let channels = Channels::new().with(held).with(channel);
    start(&path, poll(&upstream).await, channels).await;
    wait_until_stored(&path, AlertCode::Open).await;

    upstream.serve("code_7_closed.json");
    wait_until_stored(&path, AlertCode::Closed).await;
    upstream.serve("code_1_open.json");
    wait_until_stored(&path, AlertCode::Open).await;

    let closure = notification(Verdict::Open, Verdict::Closed, "code_7_closed.json");
    let reopening = notification(Verdict::Closed, Verdict::Open, "code_1_open.json");
    assert_eq!(next(&mut received).await, closure);
    assert_eq!(next(&mut received).await, reopening);
    assert_eq!(next(&mut held_received).await, closure);
    assert_nothing_more(&mut held_received).await;

    released.add_permits(1);

    assert_eq!(next(&mut held_received).await, reopening);
}
