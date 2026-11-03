use std::time::Duration;

use jiff::Timestamp;
use open_parks::open_meteo::{self, Latest};
use open_parks::park::forecast::Forecast;
use reqwest::Url;
use tokio::sync::watch;

use crate::support::fake_open_meteo::{self, serving, serving_json};
use crate::support::fixtures::open_meteo_fixture;
use crate::support::timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};
use crate::support::upstream::Upstream;

const NEVER: Duration = Duration::from_secs(3600);
const OFTEN: Duration = Duration::from_millis(10);

fn fetch_time() -> Timestamp {
    "2026-11-03T09:30:00Z".parse().unwrap()
}

fn start(url: Url, interval: Duration) -> watch::Receiver<Latest> {
    let client = open_meteo::Client::new(url, GENEROUS_TIMEOUT).unwrap();
    open_meteo::start(client, interval, fetch_time).0
}

fn decoded(name: &str) -> Forecast {
    open_meteo::decode(&open_meteo_fixture(name)).unwrap()
}

async fn wait_for_forecast(latest: &mut watch::Receiver<Latest>, name: &str) {
    let expected = decoded(name);
    let published = latest.wait_for(|latest| {
        latest
            .as_ref()
            .is_some_and(|fetched| fetched.forecast == expected)
    });
    tokio::time::timeout(GENEROUS_TIMEOUT, published)
        .await
        .unwrap_or_else(|_| panic!("{name} was never published"))
        .unwrap();
}

#[tokio::test]
async fn there_is_no_forecast_while_the_first_fetch_is_under_way() {
    let url = serving(std::future::pending::<()>).await;

    let latest = start(url, NEVER);

    assert_eq!(*latest.borrow(), None);
}

#[tokio::test]
async fn the_first_forecast_is_fetched_without_waiting_for_the_interval() {
    let mut latest = start(serving_json(open_meteo_fixture("retiro.json")).await, NEVER);

    wait_for_forecast(&mut latest, "retiro.json").await;
}

#[tokio::test]
async fn a_forecast_carries_when_it_was_fetched() {
    let mut latest = start(serving_json(open_meteo_fixture("retiro.json")).await, NEVER);

    let fetched = tokio::time::timeout(GENEROUS_TIMEOUT, latest.wait_for(Option::is_some))
        .await
        .expect("a forecast was never published")
        .unwrap();

    assert_eq!(fetched.as_ref().unwrap().fetched_at, fetch_time());
}

#[tokio::test]
async fn the_forecast_is_replaced_on_the_next_successful_fetch() {
    let upstream = Upstream::start(
        fake_open_meteo::url_for,
        Some(open_meteo_fixture("retiro.json")),
    )
    .await;
    let mut latest = start(upstream.url.clone(), OFTEN);
    wait_for_forecast(&mut latest, "retiro.json").await;

    upstream.serve(Some(open_meteo_fixture("rain_chances.json")));

    wait_for_forecast(&mut latest, "rain_chances.json").await;
}

#[tokio::test]
async fn a_failed_first_fetch_leaves_no_forecast() {
    let upstream = Upstream::start(fake_open_meteo::url_for, None).await;
    let latest = start(upstream.url.clone(), OFTEN);

    upstream.wait_for_requests(2).await;

    assert_eq!(*latest.borrow(), None);
}

#[tokio::test]
async fn a_failed_fetch_keeps_the_last_good_forecast() {
    let upstream = Upstream::start(
        fake_open_meteo::url_for,
        Some(open_meteo_fixture("retiro.json")),
    )
    .await;
    let mut latest = start(upstream.url.clone(), OFTEN);
    wait_for_forecast(&mut latest, "retiro.json").await;
    let last_good = latest.borrow().clone();

    upstream.serve(None);
    upstream.wait_for_requests(upstream.requests() + 2).await;

    assert_eq!(*latest.borrow(), last_good);
}

#[tokio::test]
async fn polling_stops_once_nothing_reads_the_forecast() {
    let upstream = Upstream::start(
        fake_open_meteo::url_for,
        Some(open_meteo_fixture("retiro.json")),
    )
    .await;
    let client = open_meteo::Client::new(upstream.url.clone(), SHORT_TIMEOUT).unwrap();
    let (latest, _) = open_meteo::start(client, OFTEN, fetch_time);
    upstream.wait_for_requests(1).await;

    drop(latest);
    tokio::time::sleep(OFTEN + SHORT_TIMEOUT).await;
    let requests = upstream.requests();
    tokio::time::sleep(SHORT_TIMEOUT).await;

    assert_eq!(upstream.requests(), requests);
}
