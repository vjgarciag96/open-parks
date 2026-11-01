use jiff::Timestamp;
use serde::Deserialize;

use crate::forecast::{Forecast, HourForecast};
use crate::http::from_json;
use crate::schedule::madrid;

mod client;
pub use client::{Client, FetchError};

pub fn decode(json: &str) -> Result<Forecast, DecodeError> {
    let Response { hourly } = from_json(json)?;
    let expected = hourly.time.len();
    for (variable, found) in [
        ("temperature_2m", hourly.temperature_2m.len()),
        (
            "precipitation_probability",
            hourly.precipitation_probability.len(),
        ),
        ("wind_gusts_10m", hourly.wind_gusts_10m.len()),
        ("weather_code", hourly.weather_code.len()),
        ("is_day", hourly.is_day.len()),
    ] {
        if found != expected {
            return Err(DecodeError::LengthMismatch {
                variable,
                expected,
                found,
            });
        }
    }

    let hours = hourly
        .time
        .iter()
        .enumerate()
        .map(|(i, UnixTime(at))| HourForecast {
            at: at.to_zoned(madrid()),
            temperature: hourly.temperature_2m[i],
            weather_code: hourly.weather_code[i],
            is_day: hourly.is_day[i].map(|IsDay(is_day)| is_day),
            // Open-Meteo gives these for the hour before each timestamp, so
            // the hour from H takes them from H+1.
            rain_chance: next(&hourly.precipitation_probability, i),
            gusts: next(&hourly.wind_gusts_10m, i),
        })
        .collect();
    Ok(Forecast { hours })
}

fn next<T: Copy>(values: &[Option<T>], i: usize) -> Option<T> {
    values.get(i + 1).copied().flatten()
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("malformed response")]
    Malformed(#[from] serde_path_to_error::Error<serde_json::Error>),
    #[error("the response has {found} values of {variable} for {expected} timestamps")]
    LengthMismatch {
        variable: &'static str,
        expected: usize,
        found: usize,
    },
}

#[derive(Deserialize)]
struct Response {
    hourly: Hourly,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<UnixTime>,
    temperature_2m: Vec<Option<f64>>,
    precipitation_probability: Vec<Option<u8>>,
    wind_gusts_10m: Vec<Option<f64>>,
    weather_code: Vec<Option<u8>>,
    is_day: Vec<Option<IsDay>>,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct UnixTime(#[serde(with = "jiff::fmt::serde::timestamp::second::required")] Timestamp);

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "u8")]
struct IsDay(bool);

impl TryFrom<u8> for IsDay {
    type Error = String;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        match raw {
            0 => Ok(IsDay(false)),
            1 => Ok(IsDay(true)),
            other => Err(format!("is_day is {other}, expected 0 or 1")),
        }
    }
}
