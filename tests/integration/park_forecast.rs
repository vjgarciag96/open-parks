use open_parks::open_meteo;

use crate::support::fixtures::open_meteo_fixture;

fn next_hours(fixture_name: &str, now: &str) -> Vec<String> {
    open_meteo::decode(&open_meteo_fixture(fixture_name))
        .unwrap()
        .next_hours(now.parse().unwrap())
        .iter()
        .map(|hour| hour.at.to_string())
        .collect()
}

#[test]
fn twelve_consecutive_hours_start_with_the_one_containing_now() {
    let hours = next_hours("retiro.json", "2026-11-02T09:30:00+01:00");

    assert_eq!(
        hours,
        [
            "2026-11-02T09:00:00+01:00[Europe/Madrid]",
            "2026-11-02T10:00:00+01:00[Europe/Madrid]",
            "2026-11-02T11:00:00+01:00[Europe/Madrid]",
            "2026-11-02T12:00:00+01:00[Europe/Madrid]",
            "2026-11-02T13:00:00+01:00[Europe/Madrid]",
            "2026-11-02T14:00:00+01:00[Europe/Madrid]",
            "2026-11-02T15:00:00+01:00[Europe/Madrid]",
            "2026-11-02T16:00:00+01:00[Europe/Madrid]",
            "2026-11-02T17:00:00+01:00[Europe/Madrid]",
            "2026-11-02T18:00:00+01:00[Europe/Madrid]",
            "2026-11-02T19:00:00+01:00[Europe/Madrid]",
            "2026-11-02T20:00:00+01:00[Europe/Madrid]",
        ]
    );
}

#[test]
fn an_hour_contains_its_last_second() {
    let hours = next_hours("retiro.json", "2026-11-02T09:59:59+01:00");

    assert_eq!(hours[0], "2026-11-02T09:00:00+01:00[Europe/Madrid]");
}

#[test]
fn the_moment_an_hour_ends_the_next_one_contains_now() {
    let hours = next_hours("retiro.json", "2026-11-02T10:00:00+01:00");

    assert_eq!(hours[0], "2026-11-02T10:00:00+01:00[Europe/Madrid]");
}

#[test]
fn fewer_hours_are_left_near_the_end_of_the_data() {
    let hours = next_hours("retiro.json", "2026-11-03T21:15:00+01:00");

    assert_eq!(
        hours,
        [
            "2026-11-03T21:00:00+01:00[Europe/Madrid]",
            "2026-11-03T22:00:00+01:00[Europe/Madrid]",
            "2026-11-03T23:00:00+01:00[Europe/Madrid]",
        ]
    );
}

#[test]
fn the_datas_last_hour_lasts_a_full_hour() {
    let hours = next_hours("retiro.json", "2026-11-03T23:59:59+01:00");

    assert_eq!(hours, ["2026-11-03T23:00:00+01:00[Europe/Madrid]"]);
}

#[test]
fn there_are_no_hours_once_the_data_has_ended() {
    let hours = next_hours("retiro.json", "2026-11-04T00:00:00+01:00");

    assert!(hours.is_empty(), "{hours:?}");
}

#[test]
fn there_are_no_hours_before_the_data_starts() {
    let hours = next_hours("retiro.json", "2026-11-01T23:59:59+01:00");

    assert!(hours.is_empty(), "{hours:?}");
}

#[test]
fn when_the_clocks_go_back_the_repeated_hour_is_found_by_its_offset() {
    let hours = next_hours("dst_fall_back.json", "2026-10-25T02:30:00+01:00");

    assert_eq!(hours[0], "2026-10-25T02:00:00+01:00[Europe/Madrid]");
}

#[test]
fn when_the_clocks_go_back_the_hour_comes_twice_in_the_columns() {
    let hours = next_hours("dst_fall_back.json", "2026-10-25T01:30:00+02:00");

    assert_eq!(
        hours[..3],
        [
            "2026-10-25T01:00:00+02:00[Europe/Madrid]",
            "2026-10-25T02:00:00+02:00[Europe/Madrid]",
            "2026-10-25T02:00:00+01:00[Europe/Madrid]",
        ]
    );
}

#[test]
fn when_the_clocks_go_forward_the_skipped_hour_is_not_in_the_columns() {
    let hours = next_hours("dst_spring_forward.json", "2027-03-28T01:30:00+01:00");

    assert_eq!(
        hours[..2],
        [
            "2027-03-28T01:00:00+01:00[Europe/Madrid]",
            "2027-03-28T03:00:00+02:00[Europe/Madrid]",
        ]
    );
}
