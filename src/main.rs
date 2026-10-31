use std::path::Path;
use std::time::Duration;
use std::{env, io};

use jiff::Timestamp;
use open_parks::database::Database;
use open_parks::notifier::{Channels, LastStatus};
use open_parks::public_url::PublicUrl;
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
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("binding {address}: {error}")))?;
    println!("listening on http://{}", listener.local_addr()?);
    let mut terminate = signal(SignalKind::terminate())?;
    let (latest, poller) = poller::start(sigma, REFRESH_INTERVAL, Timestamp::now).await;
    let mut bot = None;
    let mut notifier = None;
    if let Some(TelegramBot {
        client,
        username,
        database,
    }) = telegram
    {
        let subscribers = Subscribers::new(database.clone());
        let broadcast = Broadcast::new(
            client.clone(),
            subscribers.clone(),
            Pace::LIVE,
            public_url.clone(),
        );
        bot = Some(telegram::start(
            client,
            username,
            subscribers,
            latest.clone(),
            Timestamp::now,
        ));
        let channels = Channels::new().with(broadcast);
        notifier = Some(
            notifier::start(LastStatus::new(database), latest.clone(), channels)
                .await
                .map_err(|error| io::Error::other(format!("reading the last status: {error}")))?,
        );
    }
    let app = api::router(latest.clone(), Timestamp::now).merge(page::router(
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
        .await?;
    // With the poller stopped the status can't change again, so the
    // notifier finishes once the notifications it already has are sent.
    poller.abort();
    if let Some(bot) = bot {
        bot.abort();
    }
    if let Some(notifier) = notifier {
        let _ = notifier.await;
    }
    Ok(())
}

fn sigma_client() -> io::Result<sigma::Client> {
    let client = match env_var("OPEN_PARKS_SIGMA_QUERY_URL")? {
        Some(url) => {
            let url = Url::parse(&url).map_err(|error| {
                io::Error::other(format!("OPEN_PARKS_SIGMA_QUERY_URL: {error}"))
            })?;
            sigma::Client::new(url, sigma::Client::TIMEOUT)
        }
        None => sigma::Client::live(),
    };
    client.map_err(|error| io::Error::other(format!("building the SIGMA client: {error}")))
}

fn public_url() -> io::Result<Option<PublicUrl>> {
    let Some(url) = env_var("OPEN_PARKS_PUBLIC_URL")? else {
        return Ok(None);
    };
    let url = Url::parse(&url)
        .map_err(|error| io::Error::other(format!("OPEN_PARKS_PUBLIC_URL: {error}")))?;
    PublicUrl::new(url).map(Some).ok_or_else(|| {
        io::Error::other(
            "OPEN_PARKS_PUBLIC_URL must be an HTTP(S) origin, such as https://example.org",
        )
    })
}

struct TelegramBot {
    client: telegram::Client,
    username: String,
    /// Where the subscribers and the last status notified are kept.
    database: Database,
}

fn telegram_bot(username: Option<String>) -> io::Result<Option<TelegramBot>> {
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
    let database = notification_database()?;
    Ok(Some(TelegramBot {
        client,
        username,
        database,
    }))
}

fn notification_database() -> io::Result<Database> {
    let path = env_var("OPEN_PARKS_DATABASE")?.ok_or_else(|| {
        io::Error::other("OPEN_PARKS_DATABASE must be set when a notification channel is")
    })?;
    Database::open(Path::new(&path))
        .map_err(|error| io::Error::other(format!("opening the database at {path}: {error}")))
}

fn env_var(name: &str) -> io::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(io::Error::other(format!("{name}: {error}"))),
    }
}
