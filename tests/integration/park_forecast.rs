use jiff::{SignedDuration, Timestamp};
use open_parks::open_meteo;
use open_parks::park::forecast::{Forecast, HourForecast, NextHour};

use crate::support::fixtures::open_meteo_fixture;

fn next_hours(forecast: &Forecast, now: &str) -> Vec<String> {
    forecast
        .next_hours(now.parse().unwrap())
        .iter()
        .map(|item| match item {
            NextHour::Open { hour, label } => {
                format!("{label:?} {}", hour.at.strftime("%Y-%m-%dT%H:%M%:z"))
            }
            NextHour::Closed { from, until } => format!(
                "Closed {} to {}",
                from.strftime("%Y-%m-%dT%H:%M%:z"),
                until.strftime("%Y-%m-%dT%H:%M%:z"),
            ),
        })
        .collect()
}

fn fixture(name: &str) -> Forecast {
    open_meteo::decode(&open_meteo_fixture(name)).unwrap()
}

/// `count` consecutive hours from `first`, with no values.
fn hourly(first: &str, count: i64) -> Vec<HourForecast> {
    let first: Timestamp = first.parse().unwrap();
    (0..count)
        .map(|offset| HourForecast {
            at: (first + SignedDuration::from_hours(offset))
                .to_zoned(jiff::tz::get!("Europe/Madrid")),
            temperature: None,
            weather_code: None,
            is_day: None,
            rain_chance: None,
            gusts: None,
        })
        .collect()
}

#[test]
fn while_open_twelve_hours_start_with_now() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-02T09:30:00+01:00");

    assert_eq!(
        hours,
        [
            "Now 2026-11-02T09:00+01:00",
            "Time 2026-11-02T10:00+01:00",
            "Time 2026-11-02T11:00+01:00",
            "Time 2026-11-02T12:00+01:00",
            "Time 2026-11-02T13:00+01:00",
            "Time 2026-11-02T14:00+01:00",
            "Time 2026-11-02T15:00+01:00",
            "Time 2026-11-02T16:00+01:00",
            "Time 2026-11-02T17:00+01:00",
            "Time 2026-11-02T18:00+01:00",
            "Time 2026-11-02T19:00+01:00",
            "Time 2026-11-02T20:00+01:00",
        ]
    );
}

#[test]
fn an_hour_contains_its_last_second() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-02T09:59:59+01:00");

    assert_eq!(hours[0], "Now 2026-11-02T09:00+01:00");
}

#[test]
fn the_moment_an_hour_ends_the_next_one_contains_now() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-02T10:00:00+01:00");

    assert_eq!(hours[0], "Now 2026-11-02T10:00+01:00");
}

#[test]
fn the_winter_night_folds_into_one_block_and_only_open_hours_count() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-02T20:30:00+01:00");

    assert_eq!(
        hours,
        [
            "Now 2026-11-02T20:00+01:00",
            "Time 2026-11-02T21:00+01:00",
            "Closed 2026-11-02T22:00+01:00 to 2026-11-03T06:00+01:00",
            "DayAndTime 2026-11-03T06:00+01:00",
            "Time 2026-11-03T07:00+01:00",
            "Time 2026-11-03T08:00+01:00",
            "Time 2026-11-03T09:00+01:00",
            "Time 2026-11-03T10:00+01:00",
            "Time 2026-11-03T11:00+01:00",
            "Time 2026-11-03T12:00+01:00",
            "Time 2026-11-03T13:00+01:00",
            "Time 2026-11-03T14:00+01:00",
            "Time 2026-11-03T15:00+01:00",
        ]
    );
}

#[test]
fn the_summer_night_folds_into_one_block_from_midnight() {
    let forecast = Forecast {
        hours: hourly("2026-07-03T23:00:00+02:00", 24),
    };

    let hours = next_hours(&forecast, "2026-07-03T23:10:00+02:00");

    assert_eq!(
        hours,
        [
            "Now 2026-07-03T23:00+02:00",
            "Closed 2026-07-04T00:00+02:00 to 2026-07-04T06:00+02:00",
            "DayAndTime 2026-07-04T06:00+02:00",
            "Time 2026-07-04T07:00+02:00",
            "Time 2026-07-04T08:00+02:00",
            "Time 2026-07-04T09:00+02:00",
            "Time 2026-07-04T10:00+02:00",
            "Time 2026-07-04T11:00+02:00",
            "Time 2026-07-04T12:00+02:00",
            "Time 2026-07-04T13:00+02:00",
            "Time 2026-07-04T14:00+02:00",
            "Time 2026-07-04T15:00+02:00",
            "Time 2026-07-04T16:00+02:00",
        ]
    );
}

#[test]
fn while_closed_after_midnight_the_hours_start_at_opening_time_without_the_day() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-03T03:00:00+01:00");

    assert_eq!(
        hours,
        [
            "Time 2026-11-03T06:00+01:00",
            "Time 2026-11-03T07:00+01:00",
            "Time 2026-11-03T08:00+01:00",
            "Time 2026-11-03T09:00+01:00",
            "Time 2026-11-03T10:00+01:00",
            "Time 2026-11-03T11:00+01:00",
            "Time 2026-11-03T12:00+01:00",
            "Time 2026-11-03T13:00+01:00",
            "Time 2026-11-03T14:00+01:00",
            "Time 2026-11-03T15:00+01:00",
            "Time 2026-11-03T16:00+01:00",
            "Time 2026-11-03T17:00+01:00",
        ]
    );
}

#[test]
fn while_closed_before_midnight_the_hours_start_at_opening_time_with_the_day() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-02T23:00:00+01:00");

    assert_eq!(
        hours[..2],
        [
            "DayAndTime 2026-11-03T06:00+01:00",
            "Time 2026-11-03T07:00+01:00"
        ]
    );
}

#[test]
fn a_gap_in_the_data_during_the_night_leaves_the_block_from_closing_to_opening_time() {
    let forecast = Forecast {
        hours: [
            hourly("2026-11-02T20:00:00+01:00", 1),
            hourly("2026-11-03T08:00:00+01:00", 1),
        ]
        .concat(),
    };

    let hours = next_hours(&forecast, "2026-11-02T20:30:00+01:00");

    assert_eq!(
        hours,
        [
            "Now 2026-11-02T20:00+01:00",
            "Closed 2026-11-02T22:00+01:00 to 2026-11-03T06:00+01:00",
            "DayAndTime 2026-11-03T08:00+01:00",
        ]
    );
}

#[test]
fn when_the_clocks_go_back_the_night_folds_into_one_block() {
    let forecast = Forecast {
        hours: hourly("2026-10-24T21:00:00+02:00", 12),
    };

    let hours = next_hours(&forecast, "2026-10-24T21:30:00+02:00");

    assert_eq!(
        hours,
        [
            "Now 2026-10-24T21:00+02:00",
            "Closed 2026-10-24T22:00+02:00 to 2026-10-25T06:00+01:00",
            "DayAndTime 2026-10-25T06:00+01:00",
            "Time 2026-10-25T07:00+01:00",
        ]
    );
}

#[test]
fn when_the_clocks_go_back_during_the_night_the_hours_start_at_opening_time() {
    let hours = next_hours(&fixture("dst_fall_back.json"), "2026-10-25T02:30:00+01:00");

    assert_eq!(
        hours[..2],
        ["Time 2026-10-25T06:00+01:00", "Time 2026-10-25T07:00+01:00"]
    );
}

#[test]
fn when_the_clocks_go_forward_during_the_night_the_hours_start_at_opening_time() {
    let hours = next_hours(
        &fixture("dst_spring_forward.json"),
        "2027-03-28T01:30:00+01:00",
    );

    assert_eq!(
        hours[..2],
        ["Time 2027-03-28T06:00+02:00", "Time 2027-03-28T07:00+02:00"]
    );
}

#[test]
fn no_block_is_left_at_the_end_when_the_data_runs_out_at_night() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-03T21:15:00+01:00");

    assert_eq!(hours, ["Now 2026-11-03T21:00+01:00"]);
}

#[test]
fn while_closed_there_are_no_hours_when_the_data_ends_before_opening_time() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-03T23:30:00+01:00");

    assert!(hours.is_empty(), "{hours:?}");
}

#[test]
fn while_open_there_are_no_hours_before_the_data_starts() {
    let hours = next_hours(&fixture("retiro.json"), "2026-11-01T21:59:59+01:00");

    assert!(hours.is_empty(), "{hours:?}");
}
