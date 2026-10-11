use std::error::Error;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::park_status::{AlertCode, ParkStatus, ReopeningForecast, Verdict};
use crate::sigma::{self, FetchError};

pub fn router(sigma: sigma::Client) -> Router {
    Router::new()
        .route("/api/v1/parks/retiro/status", get(retiro_status))
        .with_state(Arc::new(sigma))
}

async fn retiro_status(State(sigma): State<Arc<sigma::Client>>) -> Response {
    match sigma.retiro_park_status().await {
        Ok(status) => Json(StatusResponse::from(&status)).into_response(),
        Err(error) => upstream_failure(&error),
    }
}

fn upstream_failure(error: &FetchError) -> Response {
    eprintln!("fetching Retiro's status: {}", with_sources(error));
    let (status, error) = match error {
        FetchError::Timeout => (StatusCode::GATEWAY_TIMEOUT, ErrorCode::UpstreamTimeout),
        FetchError::Transport(_)
        | FetchError::HttpStatus(_)
        | FetchError::BodyTooLarge
        | FetchError::Decode(_) => (StatusCode::BAD_GATEWAY, ErrorCode::UpstreamError),
    };
    (status, Json(ErrorResponse { error })).into_response()
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

#[derive(Serialize)]
struct StatusResponse<'a> {
    code: AlertCode,
    verdict: Verdict,
    dates: Option<&'a str>,
    hours: Option<&'a str>,
    reopening: Option<ReopeningForecast>,
    notes: Option<&'a str>,
}

impl<'a> From<&'a ParkStatus> for StatusResponse<'a> {
    fn from(status: &'a ParkStatus) -> Self {
        StatusResponse {
            code: status.code,
            verdict: status.code.verdict(),
            dates: status.dates.as_deref(),
            hours: status.hours.as_deref(),
            reopening: status.reopening,
            notes: status.notes.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorCode,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    UpstreamTimeout,
    UpstreamError,
}
