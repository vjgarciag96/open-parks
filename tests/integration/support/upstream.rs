use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use reqwest::Url;

use crate::support::timeouts::GENEROUS_TIMEOUT;

const OFTEN: Duration = Duration::from_millis(10);

/// A fake API whose JSON body can be changed between requests, with no
/// body meaning HTTP 503, and that counts the requests it gets.
pub struct Upstream {
    pub url: Url,
    body: Arc<Mutex<Option<String>>>,
    requests: Arc<AtomicUsize>,
}

impl Upstream {
    /// Serves at the path `url_for` gives, as the fake for that API does.
    pub async fn start(url_for: fn(SocketAddr) -> Url, body: Option<String>) -> Self {
        let body = Arc::new(Mutex::new(body));
        let requests = Arc::new(AtomicUsize::new(0));
        let handler = {
            let body = Arc::clone(&body);
            let requests = Arc::clone(&requests);
            || async move {
                requests.fetch_add(1, Ordering::SeqCst);
                match body.lock().unwrap().clone() {
                    Some(body) => {
                        ([(header::CONTENT_TYPE, "application/json")], body).into_response()
                    }
                    None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                }
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = url_for(listener.local_addr().unwrap());
        let router = Router::new().route(url.path(), get(handler));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            url,
            body,
            requests,
        }
    }

    pub fn serve(&self, body: Option<String>) {
        *self.body.lock().unwrap() = body;
    }

    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    // A poller fetches one at a time, so once `count` requests have come in,
    // every fetch before the last has been published.
    pub async fn wait_for_requests(&self, count: usize) {
        let seen = async {
            while self.requests() < count {
                tokio::time::sleep(OFTEN).await;
            }
        };
        tokio::time::timeout(GENEROUS_TIMEOUT, seen)
            .await
            .unwrap_or_else(|_| panic!("{} never saw {count} requests", self.url));
    }
}
