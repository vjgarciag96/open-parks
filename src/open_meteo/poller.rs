use std::time::Duration;

use jiff::Timestamp;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::core::clock::Clock;
use crate::core::error::with_sources;
use crate::open_meteo;
use crate::park::forecast::Forecast;

/// The last forecast fetched, or none until one is.
pub type Latest = Option<Fetched>;

#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    pub forecast: Forecast,
    pub fetched_at: Timestamp,
}

/// Fetches at once in the background, then every `interval`, until every
/// receiver is dropped or the task is aborted.
pub fn start(
    open_meteo: open_meteo::Client,
    interval: Duration,
    clock: impl Clock,
) -> (watch::Receiver<Latest>, JoinHandle<()>) {
    let (latest, receiver) = watch::channel(None);
    let task = tokio::spawn(async move {
        loop {
            if let Some(fetched) = fetch(&open_meteo, &clock).await {
                latest.send_replace(Some(fetched));
            }
            tokio::time::sleep(interval).await;
            if latest.is_closed() {
                return;
            }
        }
    });
    (receiver, task)
}

async fn fetch(open_meteo: &open_meteo::Client, clock: &impl Clock) -> Option<Fetched> {
    match open_meteo.retiro_forecast().await {
        Ok(forecast) => Some(Fetched {
            forecast,
            fetched_at: clock.now(),
        }),
        Err(error) => {
            eprintln!("fetching Retiro's forecast: {}", with_sources(&error));
            None
        }
    }
}
