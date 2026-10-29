use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use jiff::{Timestamp, Zoned};
use serde::{Serialize, Serializer};
use tokio::sync::watch;

use crate::clock::Clock;
use crate::park_status::{AlertCode, ReopeningForecast, Verdict};
use crate::poller::{Fetched, Latest};
use crate::schedule::{self, Schedule};
use crate::sigma::FetchError;

pub fn router(latest: watch::Receiver<Latest>, clock: impl Clock) -> Router {
    let clock: Arc<dyn Clock> = Arc::new(clock);
    Router::new()
        .route(
            "/api/v1/parks/retiro/status",
            get(async move |State(latest)| retiro_status(&latest, clock.now())),
        )
        .with_state(latest)
}

fn retiro_status(latest: &watch::Receiver<Latest>, now: Timestamp) -> Response {
    let schedule = ScheduleResponse::from(schedule::retiro(now));
    match &*latest.borrow() {
        Ok(fetched) => Json(StatusResponse::new(fetched, schedule)).into_response(),
        Err(error) => upstream_failure(error, schedule),
    }
}

fn upstream_failure(error: &FetchError, schedule: ScheduleResponse) -> Response {
    let code = match error {
        FetchError::Timeout => ErrorCode::UpstreamTimeout,
        FetchError::Transport(_)
        | FetchError::HttpStatus(_)
        | FetchError::BodyTooLarge
        | FetchError::Decode(_) => ErrorCode::UpstreamError,
    };
    let response = ErrorResponse {
        error: code,
        schedule,
    };
    (error.gateway_status(), Json(response)).into_response()
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
    schedule: ScheduleResponse,
}

impl<'a> StatusResponse<'a> {
    fn new(fetched: &'a Fetched, schedule: ScheduleResponse) -> Self {
        let status = &fetched.status;
        StatusResponse {
            code: status.code,
            verdict: status.code.verdict(),
            dates: status.dates.as_deref(),
            hours: status.hours.as_deref(),
            reopening: status.reopening,
            notes: status.notes.as_deref(),
            fetched_at: fetched.fetched_at_to_the_second(),
            schedule,
        }
    }
}

#[derive(Serialize)]
struct ScheduleResponse {
    open_now: bool,
    opens_at: Option<WithOffset>,
    closes_at: Option<WithOffset>,
}

impl From<Schedule> for ScheduleResponse {
    fn from(schedule: Schedule) -> Self {
        match schedule {
            Schedule::Open { closes_at } => ScheduleResponse {
                open_now: true,
                opens_at: None,
                closes_at: Some(WithOffset(closes_at)),
            },
            Schedule::Closed { opens_at } => ScheduleResponse {
                open_now: false,
                opens_at: Some(WithOffset(opens_at)),
                closes_at: None,
            },
        }
    }
}

struct WithOffset(Zoned);

impl Serialize for WithOffset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0.timestamp().display_with_offset(self.0.offset()))
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorCode,
    schedule: ScheduleResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    UpstreamTimeout,
    UpstreamError,
}
