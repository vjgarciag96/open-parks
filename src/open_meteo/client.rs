use std::time::Duration;

use reqwest::{StatusCode, Url};

use super::{DecodeError, decode};
use crate::forecast::Forecast;
use crate::http;

const FORECAST_ENDPOINT: &str = "https://api.open-meteo.com/v1/forecast";
// Retiro's centre.
const LATITUDE: &str = "40.4153";
const LONGITUDE: &str = "-3.6845";
const HOURLY: &str = "temperature_2m,precipitation_probability,wind_gusts_10m,weather_code,is_day";
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    retiro_forecast: Url,
}

impl Client {
    /// What `live` waits for each request.
    pub const TIMEOUT: Duration = Duration::from_secs(10);

    pub fn live() -> Result<Self, reqwest::Error> {
        let forecast_endpoint = Url::parse(FORECAST_ENDPOINT).expect("FORECAST_ENDPOINT is valid");
        Self::new(forecast_endpoint, Self::TIMEOUT)
    }

    pub fn new(forecast_endpoint: Url, timeout: Duration) -> Result<Self, reqwest::Error> {
        let mut retiro_forecast = forecast_endpoint;
        retiro_forecast
            .query_pairs_mut()
            .append_pair("latitude", LATITUDE)
            .append_pair("longitude", LONGITUDE)
            .append_pair("hourly", HOURLY)
            .append_pair("models", "best_match")
            .append_pair("forecast_days", "2")
            .append_pair("timeformat", "unixtime")
            // Starts the two days at Madrid's midnight, not UTC's.
            .append_pair("timezone", "Europe/Madrid");
        let http = http::client_builder().timeout(timeout).build()?;
        Ok(Self {
            http,
            retiro_forecast,
        })
    }

    pub async fn retiro_forecast(&self) -> Result<Forecast, FetchError> {
        let mut response = self.http.get(self.retiro_forecast.clone()).send().await?;
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
    #[error("request to Open-Meteo timed out")]
    Timeout,
    #[error("request to Open-Meteo failed")]
    Transport(#[source] reqwest::Error),
    #[error("Open-Meteo responded with HTTP {0}")]
    HttpStatus(StatusCode),
    #[error("Open-Meteo's response is over {MAX_BODY_BYTES} bytes")]
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
