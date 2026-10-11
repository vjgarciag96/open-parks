use std::error::Error;
use std::time::Duration;

use jiff::Timestamp;
use tokio::sync::watch;

use crate::clock::Clock;
use crate::park_status::ParkStatus;
use crate::sigma::{self, FetchError};

/// The last status fetched, or, until one is, why the latest fetch failed.
pub type Latest = Result<Fetched, FetchError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub status: ParkStatus,
    pub fetched_at: Timestamp,
}

pub async fn start(
    sigma: sigma::Client,
    interval: Duration,
    clock: impl Clock,
) -> watch::Receiver<Latest> {
    let (latest, receiver) = watch::channel(fetch(&sigma, &clock).await);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if latest.is_closed() {
                return;
            }
            let fetched = fetch(&sigma, &clock).await;
            latest.send_if_modified(|published| {
                if fetched.is_err() && published.is_ok() {
                    return false;
                }
                *published = fetched;
                true
            });
        }
    });
    receiver
}

async fn fetch(sigma: &sigma::Client, clock: &impl Clock) -> Latest {
    let latest = sigma.retiro_park_status().await.map(|status| Fetched {
        status,
        fetched_at: clock.now(),
    });
    if let Err(error) = &latest {
        eprintln!("fetching Retiro's status: {}", with_sources(error));
    }
    latest
}

pub(crate) fn with_sources(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
}
