use std::path::Path;
use std::time::Duration;
use std::{env, io};

use jiff::Timestamp;
use open_parks::database::Database;
use open_parks::notifier::{Channels, LastStatus};
use open_parks::telegram::{BotToken, Broadcast, Pace, Subscribers};
use open_parks::{api, notifier, page, poller, sigma, telegram};
use reqwest::Url;
use tokio::signal::unix::{SignalKind, signal};

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> io::Result<()> {
    let address = env_var("OPEN_PARKS_ADDRESS")?.unwrap_or_else(|| DEFAULT_ADDRESS.to_owned());
    let sigma = sigma_client()?;
    let public_url = public_url()?;
    let telegram_bot_username = env_var("OPEN_PARKS_TELEGRAM_BOT_USERNAME")?;
    let telegram = telegram_bot(telegram_bot_username.clone())?;
    let database = notification_database(telegram.is_some())?;
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("binding {address}: {error}")))?;
    println!("listening on http://{}", listener.local_addr()?);
    let mut terminate = signal(SignalKind::terminate())?;
    let latest = poller::start(sigma, REFRESH_INTERVAL, Timestamp::now).await;
    if let Some(database) = database {
        let mut channels = Channels::new();
        if let Some((client, username)) = telegram {
            let subscribers = Subscribers::new(database.clone());
            let broadcast = Broadcast::new(client.clone(), subscribers.clone(), Pace::LIVE);
            channels = channels.with(broadcast);
            telegram::start(client, username, subscribers, latest.clone());
        }
        notifier::start(LastStatus::new(database), latest.clone(), channels)
            .await
            .map_err(|error| io::Error::other(format!("reading the last status: {error}")))?;
    }
    let app = api::router(latest.clone()).merge(page::router(
        latest,
        Timestamp::now,
        telegram_bot_username,
        public_url,
    ));
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        })
        .await
}

fn sigma_client() -> io::Result<sigma::Client> {
    let client = match env_var("OPEN_PARKS_SIGMA_QUERY_URL")? {
        Some(url) => {
            let url = Url::parse(&url).map_err(|error| {
                io::Error::other(format!("OPEN_PARKS_SIGMA_QUERY_URL: {error}"))
            })?;
            sigma::Client::at(url)
        }
        None => sigma::Client::live(),
    };
    client.map_err(|error| io::Error::other(format!("building the SIGMA client: {error}")))
}

fn public_url() -> io::Result<Option<page::PublicUrl>> {
    let Some(url) = env_var("OPEN_PARKS_PUBLIC_URL")? else {
        return Ok(None);
    };
    let url = Url::parse(&url)
        .map_err(|error| io::Error::other(format!("OPEN_PARKS_PUBLIC_URL: {error}")))?;
    page::PublicUrl::new(url).map(Some).ok_or_else(|| {
        io::Error::other(
            "OPEN_PARKS_PUBLIC_URL must be an HTTP(S) origin, such as https://example.org",
        )
    })
}

fn telegram_bot(username: Option<String>) -> io::Result<Option<(telegram::Client, String)>> {
    let Some(token) = env_var("OPEN_PARKS_TELEGRAM_BOT_TOKEN")?.map(BotToken::new) else {
        return Ok(None);
    };
    let username = username.ok_or_else(|| {
        io::Error::other(
            "OPEN_PARKS_TELEGRAM_BOT_USERNAME must be set when OPEN_PARKS_TELEGRAM_BOT_TOKEN is",
        )
    })?;
    let client = telegram::Client::live(token)
        .map_err(|error| io::Error::other(format!("building the Telegram client: {error}")))?;
    Ok(Some((client, username)))
}

fn notification_database(needed: bool) -> io::Result<Option<Database>> {
    if !needed {
        return Ok(None);
    }
    let path = env_var("OPEN_PARKS_DATABASE")?.ok_or_else(|| {
        io::Error::other("OPEN_PARKS_DATABASE must be set when a notification channel is")
    })?;
    Database::open(Path::new(&path))
        .map(Some)
        .map_err(|error| io::Error::other(format!("opening the database at {path}: {error}")))
}

fn env_var(name: &str) -> io::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(io::Error::other(format!("{name}: {error}"))),
    }
}
