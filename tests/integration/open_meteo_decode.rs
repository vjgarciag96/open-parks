use jiff::Zoned;
use jiff::civil::DateTime;
use open_parks::forecast::HourForecast;
use open_parks::open_meteo::{self, DecodeError};

use crate::support::fixtures::open_meteo_fixture;

fn madrid(datetime: &str) -> Zoned {
    let datetime: DateTime = datetime.parse().unwrap();
    datetime.to_zoned(jiff::tz::get!("Europe/Madrid")).unwrap()
}

#[test]
fn every_hour_decodes_in_order_in_madrid_time() {
    let forecast = open_meteo::decode(&open_meteo_fixture("retiro.json")).unwrap();

    assert_eq!(forecast.hours.len(), 48);
    assert_eq!(
        forecast.hours[0],
        HourForecast {
            at: madrid("2026-11-02T00:00"),
            temperature: Some(9.5),
            weather_code: Some(0),
            is_day: Some(false),
            rain_chance: Some(0),
            gusts: Some(9.4),
        }
    );
    assert_eq!(forecast.hours[1].at, madrid("2026-11-02T01:00"));
    assert_eq!(forecast.hours[47].at, madrid("2026-11-03T23:00"));
    assert_eq!(
        forecast.hours[47].at.to_string(),
        "2026-11-03T23:00:00+01:00[Europe/Madrid]"
    );
}

#[test]
fn rain_chance_and_gusts_come_from_the_next_timestamp() {
    let forecast = open_meteo::decode(&open_meteo_fixture("rain_chances.json")).unwrap();

    let hour = &forecast.hours[2];
    assert_eq!(hour.at, madrid("2026-11-02T02:00"));
    assert_eq!(hour.temperature, Some(8.0));
    assert_eq!(hour.rain_chance, Some(21));
    assert_eq!(hour.gusts, Some(10.1));
}

#[test]
fn the_last_hour_has_no_rain_chance_or_gusts() {
    let forecast = open_meteo::decode(&open_meteo_fixture("rain_chances.json")).unwrap();

    assert_eq!(
        forecast.hours.last(),
        Some(&HourForecast {
            at: madrid("2026-11-03T23:00"),
            temperature: Some(7.5),
            weather_code: Some(0),
            is_day: Some(false),
            rain_chance: None,
            gusts: None,
        })
    );
}

#[test]
fn null_values_decode_as_missing() {
    let forecast = open_meteo::decode(&open_meteo_fixture("nulls.json")).unwrap();

    assert_eq!(
        forecast.hours[0],
        HourForecast {
            at: madrid("2026-11-02T00:00"),
            temperature: None,
            weather_code: None,
            is_day: None,
            rain_chance: None,
            gusts: None,
        }
    );
    assert_eq!(forecast.hours[1].rain_chance, Some(0));
    assert_eq!(forecast.hours[1].gusts, Some(9.7));
}

#[test]
fn is_day_decodes_as_day_or_night() {
    let forecast = open_meteo::decode(&open_meteo_fixture("retiro.json")).unwrap();

    assert_eq!(forecast.hours[0].is_day, Some(false));
    assert_eq!(forecast.hours[12].is_day, Some(true));
}

#[test]
fn an_is_day_other_than_0_or_1_is_a_decode_error() {
    let result = open_meteo::decode(&open_meteo_fixture("is_day_2.json"));

    let Err(DecodeError::Malformed(error)) = result else {
        panic!("got {result:?}");
    };
    assert_eq!(error.path().to_string(), "hourly.is_day[3]");
}

#[test]
fn a_variable_with_fewer_values_than_timestamps_is_a_decode_error() {
    let result = open_meteo::decode(&open_meteo_fixture("mismatched_lengths.json"));

    assert!(
        matches!(
            result,
            Err(DecodeError::LengthMismatch {
                variable: "wind_gusts_10m",
                expected: 48,
                found: 47,
            })
        ),
        "got {result:?}"
    );
}

#[test]
fn a_malformed_body_is_a_decode_error() {
    let result = open_meteo::decode(&open_meteo_fixture("malformed.json"));

    assert!(
        matches!(result, Err(DecodeError::Malformed(_))),
        "got {result:?}"
    );
}
