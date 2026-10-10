use std::sync::Arc;
use std::time::Duration;

use reqwest::{StatusCode, Url};

use super::{DecodeError, decode};
use crate::park_status::ParkStatus;

const QUERY_ENDPOINT: &str =
    "https://sigma.madrid.es/hosted/rest/services/MEDIO_AMBIENTE/ALERTAS_PARQUES/MapServer/0/query";
const TIMEOUT: Duration = Duration::from_secs(10);
const RETIRO: &str = "Jardines del Buen Retiro";
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    retiro_query: Url,
}

impl Client {
    pub fn live() -> Result<Self, reqwest::Error> {
        Self::new(
            Url::parse(QUERY_ENDPOINT).expect("QUERY_ENDPOINT is valid"),
            TIMEOUT,
        )
    }

    pub fn new(query_endpoint: Url, timeout: Duration) -> Result<Self, reqwest::Error> {
        let mut retiro_query = query_endpoint;
        retiro_query
            .query_pairs_mut()
            .append_pair("where", &format!("ZONA_VERDE='{RETIRO}'"))
            .append_pair("outFields", "*")
            .append_pair("returnGeometry", "false")
            .append_pair("f", "json");
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(tls_config())
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(timeout)
            .build()?;
        Ok(Self { http, retiro_query })
    }

    pub async fn retiro_park_status(&self) -> Result<ParkStatus, FetchError> {
        let mut response = self.http.get(self.retiro_query.clone()).send().await?;
        let http_status = response.status();
        if !http_status.is_success() {
            return Err(FetchError::HttpStatus(http_status));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_BODY_BYTES {
                return Err(FetchError::BodyTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(decode(&String::from_utf8_lossy(&body))?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("request to SIGMA timed out")]
    Timeout,
    #[error("request to SIGMA failed")]
    Transport(#[source] reqwest::Error),
    #[error("SIGMA responded with HTTP {0}")]
    HttpStatus(StatusCode),
    #[error("SIGMA's response is over {MAX_BODY_BYTES} bytes")]
    BodyTooLarge,
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

impl From<reqwest::Error> for FetchError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            FetchError::Timeout
        } else {
            FetchError::Transport(error)
        }
    }
}

fn tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}
