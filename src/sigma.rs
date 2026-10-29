use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::park_status::{AlertCode, ParkStatus, ReopeningForecast};

mod client;
pub use client::{Client, FetchError};

pub fn decode(json: &str) -> Result<ParkStatus, DecodeError> {
    // SIGMA can send an error body with a 200 status. Checking for one first
    // lets the query response decode strictly, so errors name the bad field.
    if let ErrorBody {
        error: Some(SigmaError { code, message }),
    } = from_json(json)?
    {
        return Err(DecodeError::Upstream { code, message });
    }

    let QueryResponse { mut features } = from_json(json)?;
    let Feature { attributes } = match features.len() {
        0 => return Err(DecodeError::NoFeatures),
        1 => features.remove(0),
        n => return Err(DecodeError::MultipleFeatures(n)),
    };
    Ok(ParkStatus {
        code: alert_code(attributes.alert_code),
        dates: non_blank(attributes.dates),
        hours: non_blank(attributes.hours),
        reopening: attributes.reopening.map(reopening_forecast),
        notes: non_blank(attributes.notes),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("the response has no features")]
    NoFeatures,
    #[error("the response has {0} features, expected one")]
    MultipleFeatures(usize),
    #[error("malformed response")]
    Malformed(#[from] serde_path_to_error::Error<serde_json::Error>),
    #[error("SIGMA error {code}: {message}")]
    Upstream { code: i32, message: String },
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<SigmaError>,
}

#[derive(Deserialize)]
struct SigmaError {
    code: i32,
    message: String,
}

#[derive(Deserialize)]
struct QueryResponse {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Feature {
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    #[serde(rename = "ALERTA_DESCRIPCION")]
    alert_code: i16,
    #[serde(rename = "FECHA_INCIDENCIA")]
    dates: Option<String>,
    #[serde(rename = "HORARIO_INCIDENCIA")]
    hours: Option<String>,
    #[serde(rename = "PREVISION_APERTURA")]
    reopening: Option<i16>,
    #[serde(rename = "OBSERVACIONES")]
    notes: Option<String>,
}

fn from_json<T: DeserializeOwned>(
    json: &str,
) -> Result<T, serde_path_to_error::Error<serde_json::Error>> {
    let mut track = serde_path_to_error::Track::new();
    let mut deserializer = serde_json::Deserializer::from_str(json);
    T::deserialize(serde_path_to_error::Deserializer::new(
        &mut deserializer,
        &mut track,
    ))
    // Reject anything after the JSON value, as `serde_json::from_str` does.
    .and_then(|value| deserializer.end().map(|()| value))
    .map_err(|error| serde_path_to_error::Error::new(track.path(), error))
}

// Codes from the layer's renderer (`MapServer/0?f=json`).
pub(crate) fn alert_code(raw: i16) -> AlertCode {
    match raw {
        1 => AlertCode::Open,
        2 => AlertCode::Incidents,
        3 => AlertCode::YellowAlert,
        4 => AlertCode::OrangeAlert,
        5 => AlertCode::RedAlertClosure,
        6 => AlertCode::RedAlertRestrictions,
        7 => AlertCode::Closed,
        other => AlertCode::Unknown(other),
    }
}

pub(crate) fn raw_alert_code(code: AlertCode) -> i16 {
    match code {
        AlertCode::Open => 1,
        AlertCode::Incidents => 2,
        AlertCode::YellowAlert => 3,
        AlertCode::OrangeAlert => 4,
        AlertCode::RedAlertClosure => 5,
        AlertCode::RedAlertRestrictions => 6,
        AlertCode::Closed => 7,
        AlertCode::Unknown(raw) => raw,
    }
}

// Codes from the field's coded-value domain (`MapServer/0?f=json`).
fn reopening_forecast(raw: i16) -> ReopeningForecast {
    match raw {
        1 => ReopeningForecast::AfterInspections,
        2 => ReopeningForecast::SeeNotes,
        other => ReopeningForecast::Unknown(other),
    }
}

fn non_blank(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}
