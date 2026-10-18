use std::time::Duration;

use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::clock::Clock;
use crate::error::with_sources;
use crate::park_status::ParkStatus;
use crate::sigma::{self, FetchError};

/// The last status fetched, or, until one is, why the latest fetch failed.
pub type Latest = Result<Fetched, FetchError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub status: ParkStatus,
    pub fetched_at: Timestamp,
}

impl Fetched {
    /// When it was fetched, to the second, as the API and the page say.
    pub(crate) fn fetched_at_to_the_second(&self) -> Timestamp {
        self.fetched_at
            .round(
                TimestampRound::new()
                    .smallest(Unit::Second)
                    .mode(RoundMode::Floor),
            )
            .expect("flooring to a second stays within range")
    }
}

/// Polls until every receiver is dropped or the task is aborted.
pub async fn start(
    sigma: sigma::Client,
    interval: Duration,
    clock: impl Clock,
) -> (watch::Receiver<Latest>, JoinHandle<()>) {
    let (latest, receiver) = watch::channel(fetch(&sigma, &clock).await);
    let task = tokio::spawn(async move {
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
    (receiver, task)
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
