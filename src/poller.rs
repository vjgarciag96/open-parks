use std::error::Error;
use std::time::Duration;

use tokio::sync::watch;

use crate::park_status::ParkStatus;
use crate::sigma::{self, FetchError};

pub type Latest = Result<ParkStatus, FetchError>;

pub async fn start(sigma: sigma::Client, interval: Duration) -> watch::Receiver<Latest> {
    let (latest, receiver) = watch::channel(fetch(&sigma).await);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if latest.send(fetch(&sigma).await).is_err() {
                return;
            }
        }
    });
    receiver
}

async fn fetch(sigma: &sigma::Client) -> Latest {
    let latest = sigma.retiro_park_status().await;
    if let Err(error) = &latest {
        eprintln!("fetching Retiro's status: {}", with_sources(error));
    }
    latest
}

fn with_sources(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
}
