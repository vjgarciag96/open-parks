use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, Request};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use open_parks::open_meteo::{self, DecodeError, FetchError};
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::support::fake_open_meteo::{json, serving, serving_json, url_for};
use crate::support::fixtures::open_meteo_fixture;
use crate::support::timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};

async fn recording_requests() -> (Url, Arc<Mutex<Option<Parts>>>) {
    let received = Arc::new(Mutex::new(None));
    let body = open_meteo_fixture("retiro.json");
    let url = serving({
        let received = Arc::clone(&received);
        |request: Request| async move {
            *received.lock().unwrap() = Some(request.into_parts().0);
            json(body)
        }
    })
    .await;
    (url, received)
}

fn client(url: Url, timeout: Duration) -> open_meteo::Client {
    open_meteo::Client::new(url, timeout).unwrap()
}

#[tokio::test]
async fn retiros_forecast_is_fetched_and_decoded() {
    let url = serving_json(open_meteo_fixture("rain_chances.json")).await;

    let forecast = client(url, GENEROUS_TIMEOUT)
        .retiro_forecast()
        .await
        .unwrap();

    assert_eq!(
        forecast,
        open_meteo::decode(&open_meteo_fixture("rain_chances.json")).unwrap()
    );
}

#[tokio::test]
async fn two_days_of_retiros_hourly_variables_are_requested() {
    let (url, received) = recording_requests().await;

    client(url, GENEROUS_TIMEOUT)
        .retiro_forecast()
        .await
        .unwrap();

    let request = received
        .lock()
        .unwrap()
        .take()
        .expect("no request received");
    let Query(params) = Query::<HashMap<String, String>>::try_from_uri(&request.uri).unwrap();
    assert_eq!(
        params,
        HashMap::from(
            [
                ("latitude", "40.4153"),
                ("longitude", "-3.6845"),
                (
                    "hourly",
                    "temperature_2m,precipitation_probability,wind_gusts_10m,weather_code,is_day"
                ),
                ("models", "best_match"),
                ("forecast_days", "2"),
                ("timeformat", "unixtime"),
                ("timezone", "Europe/Madrid"),
            ]
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
        )
    );
}

#[tokio::test]
async fn requests_identify_open_parks() {
    let (url, received) = recording_requests().await;

    client(url, GENEROUS_TIMEOUT)
        .retiro_forecast()
        .await
        .unwrap();

    let request = received
        .lock()
        .unwrap()
        .take()
        .expect("no request received");
    assert_eq!(
        request.headers[header::USER_AGENT],
        format!("open-parks/{}", env!("CARGO_PKG_VERSION")).as_str()
    );
}

#[tokio::test]
async fn an_error_body_is_reported_by_its_http_status() {
    let body = open_meteo_fixture("error_body.json");
    let url = serving(|| async move { (StatusCode::BAD_REQUEST, json(body)) }).await;

    let result = client(url, GENEROUS_TIMEOUT).retiro_forecast().await;

    assert!(
        matches!(result, Err(FetchError::HttpStatus(StatusCode::BAD_REQUEST))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn an_http_error_status_is_an_error_even_with_a_decodable_body() {
    let body = open_meteo_fixture("retiro.json");
    let url = serving(|| async move { (StatusCode::SERVICE_UNAVAILABLE, json(body)) }).await;

    let result = client(url, GENEROUS_TIMEOUT).retiro_forecast().await;

    assert!(
        matches!(
            result,
            Err(FetchError::HttpStatus(StatusCode::SERVICE_UNAVAILABLE))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn an_upstream_that_never_responds_times_out() {
    let url = serving(std::future::pending::<()>).await;

    let result = client(url, SHORT_TIMEOUT).retiro_forecast().await;

    assert!(matches!(result, Err(FetchError::Timeout)), "got {result:?}");
}

#[tokio::test]
async fn a_body_that_stops_arriving_times_out() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{\"hourly\"")
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });

    let result = client(url_for(address), SHORT_TIMEOUT)
        .retiro_forecast()
        .await;

    assert!(matches!(result, Err(FetchError::Timeout)), "got {result:?}");
}

#[tokio::test]
async fn an_oversized_body_is_an_error() {
    let body = " ".repeat(8 * 1024 * 1024);
    let url = serving(|| async move { json(body) }).await;

    let result = client(url, GENEROUS_TIMEOUT).retiro_forecast().await;

    assert!(
        matches!(result, Err(FetchError::BodyTooLarge)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_malformed_body_is_a_decode_error() {
    let url = serving_json(open_meteo_fixture("malformed.json")).await;

    let result = client(url, GENEROUS_TIMEOUT).retiro_forecast().await;

    assert!(
        matches!(result, Err(FetchError::Decode(DecodeError::Malformed(_)))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn an_unreachable_upstream_is_a_transport_error() {
    let unreachable = SocketAddr::from(([127, 0, 0, 1], 0));

    let result = client(url_for(unreachable), GENEROUS_TIMEOUT)
        .retiro_forecast()
        .await;

    assert!(
        matches!(result, Err(FetchError::Transport(_))),
        "got {result:?}"
    );
}

#[test]
fn the_live_client_has_a_valid_url_and_tls_setup() {
    assert!(open_meteo::Client::live().is_ok());
}
