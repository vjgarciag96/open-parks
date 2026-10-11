use std::time::Duration;
use std::{env, io};

use open_parks::{api, poller, sigma};
use reqwest::Url;
use tokio::signal::unix::{SignalKind, signal};

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> io::Result<()> {
    let address = env_var("OPEN_PARKS_ADDRESS")?.unwrap_or_else(|| DEFAULT_ADDRESS.to_owned());
    let sigma = sigma_client()?;
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("binding {address}: {error}")))?;
    println!("listening on http://{}", listener.local_addr()?);
    let mut terminate = signal(SignalKind::terminate())?;
    let latest = poller::start(sigma, REFRESH_INTERVAL).await;
    axum::serve(listener, api::router(latest))
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

fn env_var(name: &str) -> io::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(io::Error::other(format!("{name}: {error}"))),
    }
}
