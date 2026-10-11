use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use open_parks::api;
use open_parks::park_status::AlertCode;
use open_parks::poller::{self, Latest};
use open_parks::sigma::{self, FetchError};
use reqwest::Url;
use tokio::sync::watch;
use tower::ServiceExt;

mod common;
mod fake_upstream;
mod timeouts;
use common::fixture;
use fake_upstream::{json, serving, serving_json};
use timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};

const NEVER: Duration = Duration::from_secs(3600);
const OFTEN: Duration = Duration::from_millis(10);

struct Upstream {
    url: Url,
    body: Arc<Mutex<Option<String>>>,
    requests: Arc<AtomicUsize>,
}

impl Upstream {
    async fn start(body: Option<String>) -> Self {
        let body = Arc::new(Mutex::new(body));
        let requests = Arc::new(AtomicUsize::new(0));
        let url = serving({
            let body = Arc::clone(&body);
            let requests = Arc::clone(&requests);
            || async move {
                requests.fetch_add(1, Ordering::SeqCst);
                match body.lock().unwrap().clone() {
                    Some(body) => json(body).into_response(),
                    None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                }
            }
        })
        .await;
        Self {
            url,
            body,
            requests,
        }
    }

    fn serve(&self, body: Option<String>) {
        *self.body.lock().unwrap() = body;
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

async fn start(url: Url, interval: Duration) -> watch::Receiver<Latest> {
    poller::start(sigma::Client::new(url, GENEROUS_TIMEOUT).unwrap(), interval).await
}

async fn wait_for_code(latest: &mut watch::Receiver<Latest>, code: AlertCode) {
    let published =
        latest.wait_for(|latest| latest.as_ref().is_ok_and(|status| status.code == code));
    tokio::time::timeout(GENEROUS_TIMEOUT, published)
        .await
        .unwrap_or_else(|_| panic!("{code:?} was never published"))
        .unwrap();
}

#[tokio::test]
async fn the_first_status_is_fetched_before_start_returns() {
    let latest = start(serving_json(fixture("code_7_closed.json")).await, NEVER).await;

    assert_eq!(
        *latest.borrow().as_ref().unwrap(),
        sigma::decode(&fixture("code_7_closed.json")).unwrap()
    );
}

#[tokio::test]
async fn the_status_is_refreshed_on_an_interval() {
    let upstream = Upstream::start(Some(fixture("code_1_open.json"))).await;
    let mut latest = start(upstream.url.clone(), OFTEN).await;

    upstream.serve(Some(fixture("code_7_closed.json")));

    wait_for_code(&mut latest, AlertCode::Closed).await;
}

#[tokio::test]
async fn a_failed_fetch_is_published_until_a_later_one_succeeds() {
    let upstream = Upstream::start(None).await;
    let mut latest = start(upstream.url.clone(), OFTEN).await;

    assert!(matches!(
        *latest.borrow(),
        Err(FetchError::HttpStatus(StatusCode::SERVICE_UNAVAILABLE))
    ));

    upstream.serve(Some(fixture("code_1_open.json")));

    wait_for_code(&mut latest, AlertCode::Open).await;
}

#[tokio::test]
async fn requests_are_served_without_querying_sigma() {
    let upstream = Upstream::start(Some(fixture("code_1_open.json"))).await;
    let router = api::router(start(upstream.url.clone(), NEVER).await);

    for _ in 0..3 {
        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/parks/retiro/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    assert_eq!(upstream.requests(), 1);
}

#[tokio::test]
async fn polling_stops_once_nothing_reads_the_status() {
    let upstream = Upstream::start(Some(fixture("code_1_open.json"))).await;
    let client = sigma::Client::new(upstream.url.clone(), SHORT_TIMEOUT).unwrap();
    let latest = poller::start(client, OFTEN).await;

    drop(latest);
    tokio::time::sleep(OFTEN + SHORT_TIMEOUT).await;
    let requests = upstream.requests();
    tokio::time::sleep(SHORT_TIMEOUT).await;

    assert_eq!(upstream.requests(), requests);
}
