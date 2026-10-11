use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
use serde::Serialize;
use tokio::sync::watch;

use crate::park_status::{AlertCode, ReopeningForecast, Verdict};
use crate::poller::{Fetched, Latest};
use crate::sigma::FetchError;

pub fn router(latest: watch::Receiver<Latest>) -> Router {
    Router::new()
        .route("/api/v1/parks/retiro/status", get(retiro_status))
        .with_state(latest)
}

async fn retiro_status(State(latest): State<watch::Receiver<Latest>>) -> Response {
    match &*latest.borrow() {
        Ok(fetched) => Json(StatusResponse::from(fetched)).into_response(),
        Err(error) => upstream_failure(error),
    }
}

fn upstream_failure(error: &FetchError) -> Response {
    let (status, error) = upstream_failure_status(error);
    (status, Json(ErrorResponse { error })).into_response()
}

pub(crate) fn upstream_failure_status(error: &FetchError) -> (StatusCode, ErrorCode) {
    match error {
        FetchError::Timeout => (StatusCode::GATEWAY_TIMEOUT, ErrorCode::UpstreamTimeout),
        FetchError::Transport(_)
        | FetchError::HttpStatus(_)
        | FetchError::BodyTooLarge
        | FetchError::Decode(_) => (StatusCode::BAD_GATEWAY, ErrorCode::UpstreamError),
    }
}

#[derive(Serialize)]
struct StatusResponse<'a> {
    code: AlertCode,
    verdict: Verdict,
    dates: Option<&'a str>,
    hours: Option<&'a str>,
    reopening: Option<ReopeningForecast>,
    notes: Option<&'a str>,
    fetched_at: Timestamp,
}

impl<'a> From<&'a Fetched> for StatusResponse<'a> {
    fn from(Fetched { status, fetched_at }: &'a Fetched) -> Self {
        StatusResponse {
            code: status.code,
            verdict: status.code.verdict(),
            dates: status.dates.as_deref(),
            hours: status.hours.as_deref(),
            reopening: status.reopening,
            notes: status.notes.as_deref(),
            fetched_at: to_the_second(*fetched_at),
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorCode,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    UpstreamTimeout,
    UpstreamError,
}

pub(crate) fn to_the_second(timestamp: Timestamp) -> Timestamp {
    timestamp
        .round(
            TimestampRound::new()
                .smallest(Unit::Second)
                .mode(RoundMode::Floor),
        )
        .expect("flooring to a second stays within range")
}
