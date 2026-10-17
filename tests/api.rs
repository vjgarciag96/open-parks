use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use jiff::Timestamp;
use open_parks::api;
use open_parks::poller::{self, Fetched, Latest};
use open_parks::sigma;
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::watch;
use tower::ServiceExt;

mod common;
mod fake_upstream;
mod timeouts;
use common::fixture;
use fake_upstream::{serving, serving_json, url_for};
use timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};

const RETIRO_STATUS: &str = "/api/v1/parks/retiro/status";
const NOW: &str = "2026-10-11T09:30:00Z";

async fn get(latest: watch::Receiver<Latest>, uri: &str) -> Response {
    get_at(latest, NOW.parse().unwrap(), uri).await
}

async fn get_at(latest: watch::Receiver<Latest>, now: Timestamp, uri: &str) -> Response {
    api::router(latest, move || now)
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn fetched(fixture_name: &str) -> watch::Receiver<Latest> {
    let fetched = Fetched {
        status: sigma::decode(&fixture(fixture_name)).unwrap(),
        fetched_at: "2026-10-11T09:30:00.987654321Z".parse().unwrap(),
    };
    watch::channel(Ok(fetched)).1
}

async fn get_retiro_status(fixture_name: &str) -> Response {
    get(fetched(fixture_name), RETIRO_STATUS).await
}

async fn get_retiro_status_from(upstream: Url, timeout: Duration) -> Response {
    let client = sigma::Client::new(upstream, timeout).unwrap();
    let latest = poller::start(client, Duration::from_secs(3600), Timestamp::now).await;
    get(latest, RETIRO_STATUS).await
}

fn open_until_ten_at_night() -> Value {
    json!({
        "open_now": true,
        "opens_at": null,
        "closes_at": "2026-10-11T22:00:00+02:00",
    })
}

async fn json_body(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn assert_upstream_error(response: Response) {
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        json_body(response).await,
        json!({ "error": "upstream_error", "schedule": open_until_ten_at_night() })
    );
}

#[tokio::test]
async fn retiro_status_is_served_as_json() {
    let response = get_retiro_status("code_1_open.json").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn retiro_status_is_served_as_machine_values_with_the_raw_details() {
    let body = json_body(get_retiro_status("code_6_red_alert_restrictions.json").await).await;

    assert_eq!(
        body,
        json!({
            "code": "red_alert_restrictions",
            "verdict": "open_with_restrictions",
            "dates": "12/10/2026",
            "hours": "de 14:00 a 20:00 h",
            "reopening": null,
            "notes": "Cerradas las zonas infantiles, deportivas y de mayores. \
                      Suspendidos los eventos al aire libre.",
            "fetched_at": "2026-10-11T09:30:00Z",
            "schedule": open_until_ten_at_night(),
        })
    );
}

#[tokio::test]
async fn fetched_at_is_served_to_the_second_it_was_fetched_in() {
    let body = json_body(get_retiro_status("code_1_open.json").await).await;

    assert_eq!(body["fetched_at"], json!("2026-10-11T09:30:00Z"));
}

#[tokio::test]
async fn every_alert_code_is_served_by_name_with_its_verdict() {
    let cases = [
        ("code_1_open.json", "open", "open"),
        (
            "code_2_incidents.json",
            "incidents",
            "open_with_restrictions",
        ),
        (
            "code_3_yellow_alert.json",
            "yellow_alert",
            "open_with_restrictions",
        ),
        (
            "code_4_orange_alert.json",
            "orange_alert",
            "open_stay_not_advised",
        ),
        (
            "code_5_red_alert_closure.json",
            "red_alert_closure",
            "closure_forecast",
        ),
        (
            "code_6_red_alert_restrictions.json",
            "red_alert_restrictions",
            "open_with_restrictions",
        ),
        ("code_7_closed.json", "closed", "closed"),
        ("unknown_codes.json", "unknown", "unknown"),
    ];
    for (name, code, verdict) in cases {
        let body = json_body(get_retiro_status(name).await).await;

        assert_eq!(
            (&body["code"], &body["verdict"]),
            (&json!(code), &json!(verdict)),
            "fixture {name}"
        );
    }
}

#[tokio::test]
async fn every_reopening_forecast_is_served_by_name() {
    let cases = [
        ("code_5_red_alert_closure.json", "after_inspections"),
        ("code_7_closed.json", "see_notes"),
        ("unknown_codes.json", "unknown"),
    ];
    for (name, reopening) in cases {
        let body = json_body(get_retiro_status(name).await).await;

        assert_eq!(body["reopening"], json!(reopening), "fixture {name}");
    }
}

#[tokio::test]
async fn absent_details_are_served_as_null() {
    let body = json_body(get_retiro_status("blank_strings.json").await).await;

    assert_eq!(
        body,
        json!({
            "code": "open",
            "verdict": "open",
            "dates": null,
            "hours": null,
            "reopening": null,
            "notes": null,
            "fetched_at": "2026-10-11T09:30:00Z",
            "schedule": open_until_ten_at_night(),
        })
    );
}

#[tokio::test]
async fn within_opening_hours_the_schedule_says_when_the_park_closes() {
    let response = get_at(
        fetched("code_1_open.json"),
        "2026-10-11T21:59:59+02:00".parse().unwrap(),
        RETIRO_STATUS,
    )
    .await;

    assert_eq!(
        json_body(response).await["schedule"],
        json!({
            "open_now": true,
            "opens_at": null,
            "closes_at": "2026-10-11T22:00:00+02:00",
        })
    );
}

#[tokio::test]
async fn outside_opening_hours_the_schedule_says_when_the_park_opens() {
    let response = get_at(
        fetched("code_1_open.json"),
        "2026-10-11T22:00:00+02:00".parse().unwrap(),
        RETIRO_STATUS,
    )
    .await;

    assert_eq!(
        json_body(response).await["schedule"],
        json!({
            "open_now": false,
            "opens_at": "2026-10-12T06:00:00+02:00",
            "closes_at": null,
        })
    );
}

#[tokio::test]
async fn other_parks_are_not_found() {
    let response = get(
        fetched("code_1_open.json"),
        "/api/v1/parks/casa-de-campo/status",
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_failing_upstream_is_a_bad_gateway() {
    let upstream = serving(|| async { StatusCode::SERVICE_UNAVAILABLE }).await;

    let response = get_retiro_status_from(upstream, GENEROUS_TIMEOUT).await;

    assert_upstream_error(response).await;
}

#[tokio::test]
async fn an_undecodable_upstream_response_is_a_bad_gateway() {
    let upstream = serving_json(fixture("malformed.json")).await;

    let response = get_retiro_status_from(upstream, GENEROUS_TIMEOUT).await;

    assert_upstream_error(response).await;
}

#[tokio::test]
async fn an_oversized_upstream_response_is_a_bad_gateway() {
    let upstream = serving_json(" ".repeat(8 * 1024 * 1024)).await;

    let response = get_retiro_status_from(upstream, GENEROUS_TIMEOUT).await;

    assert_upstream_error(response).await;
}

#[tokio::test]
async fn an_unreachable_upstream_is_a_bad_gateway() {
    let unreachable = url_for(SocketAddr::from(([127, 0, 0, 1], 0)));

    let response = get_retiro_status_from(unreachable, GENEROUS_TIMEOUT).await;

    assert_upstream_error(response).await;
}

#[tokio::test]
async fn an_upstream_that_never_responds_is_a_gateway_timeout() {
    let upstream = serving(std::future::pending::<()>).await;

    let response = get_retiro_status_from(upstream, SHORT_TIMEOUT).await;

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(
        json_body(response).await,
        json!({ "error": "upstream_timeout", "schedule": open_until_ten_at_night() })
    );
}
