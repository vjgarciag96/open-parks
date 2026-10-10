use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::park_status::{AlertCode, ParkStatus, ReopeningForecast, Verdict};

pub fn router(retiro: ParkStatus) -> Router {
    Router::new()
        .route("/api/v1/parks/retiro/status", get(retiro_status))
        .with_state(Arc::new(retiro))
}

async fn retiro_status(State(status): State<Arc<ParkStatus>>) -> Response {
    Json(StatusResponse::from(status.as_ref())).into_response()
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
