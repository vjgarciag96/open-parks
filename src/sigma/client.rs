use std::time::Duration;

use reqwest::{StatusCode, Url};

use super::{DecodeError, decode};
use crate::http;
use crate::park_status::ParkStatus;

const QUERY_ENDPOINT: &str =
    "https://sigma.madrid.es/hosted/rest/services/MEDIO_AMBIENTE/ALERTAS_PARQUES/MapServer/0/query";
const RETIRO: &str = "Jardines del Buen Retiro";
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    retiro_query: Url,
}

impl Client {
    /// What `live` waits for each request.
    pub const TIMEOUT: Duration = Duration::from_secs(10);

    pub fn live() -> Result<Self, reqwest::Error> {
        let query_endpoint = Url::parse(QUERY_ENDPOINT).expect("QUERY_ENDPOINT is valid");
        Self::new(query_endpoint, Self::TIMEOUT)
    }

    pub fn new(query_endpoint: Url, timeout: Duration) -> Result<Self, reqwest::Error> {
        let mut retiro_query = query_endpoint;
        retiro_query
            .query_pairs_mut()
            .append_pair("where", &format!("ZONA_VERDE='{RETIRO}'"))
            .append_pair("outFields", "*")
            .append_pair("returnGeometry", "false")
            .append_pair("f", "json");
        let http = http::client_builder().timeout(timeout).build()?;
        Ok(Self { http, retiro_query })
    }

    pub async fn retiro_park_status(&self) -> Result<ParkStatus, FetchError> {
        let mut response = self.http.get(self.retiro_query.clone()).send().await?;
        let http_status = response.status();
        if !http_status.is_success() {
            return Err(FetchError::HttpStatus(http_status));
        }
        let body = http::read_body(&mut response, MAX_BODY_BYTES, FetchError::BodyTooLarge).await?;
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

impl FetchError {
    /// What a gateway answers with when SIGMA fails this way.
    pub(crate) fn gateway_status(&self) -> StatusCode {
        match self {
            FetchError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            FetchError::Transport(_)
            | FetchError::HttpStatus(_)
            | FetchError::BodyTooLarge
            | FetchError::Decode(_) => StatusCode::BAD_GATEWAY,
        }
    }
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
