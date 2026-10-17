use open_parks::schedule::{self, Schedule};

fn at(now: &str) -> (&'static str, String) {
    match schedule::retiro(now.parse().unwrap()) {
        Schedule::Open { closes_at } => ("closes at", closes_at.to_string()),
        Schedule::Closed { opens_at } => ("opens at", opens_at.to_string()),
    }
}

fn opens(time: &str) -> (&'static str, String) {
    ("opens at", format!("{time}[Europe/Madrid]"))
}

fn closes(time: &str) -> (&'static str, String) {
    ("closes at", format!("{time}[Europe/Madrid]"))
}

#[test]
fn before_six_in_winter_the_park_opens_that_morning() {
    assert_eq!(
        at("2026-10-18T05:59:59+02:00"),
        opens("2026-10-18T06:00:00+02:00")
    );
}

#[test]
fn from_six_in_winter_the_park_closes_at_ten_at_night() {
    for now in ["2026-10-18T06:00:00+02:00", "2026-10-18T21:59:59+02:00"] {
        assert_eq!(at(now), closes("2026-10-18T22:00:00+02:00"), "at {now}");
    }
}

#[test]
fn from_ten_at_night_in_winter_the_park_opens_the_next_morning() {
    for now in ["2026-10-18T22:00:00+02:00", "2026-10-18T23:59:59+02:00"] {
        assert_eq!(at(now), opens("2026-10-19T06:00:00+02:00"), "at {now}");
    }
}

#[test]
fn in_summer_the_park_closes_at_midnight() {
    for now in [
        "2026-07-15T06:00:00+02:00",
        "2026-07-15T22:00:00+02:00",
        "2026-07-15T23:59:59+02:00",
    ] {
        assert_eq!(at(now), closes("2026-07-16T00:00:00+02:00"), "at {now}");
    }
}

#[test]
fn from_midnight_in_summer_the_park_opens_that_morning() {
    for now in ["2026-07-16T00:00:00+02:00", "2026-07-16T05:59:59+02:00"] {
        assert_eq!(at(now), opens("2026-07-16T06:00:00+02:00"), "at {now}");
    }
}

#[test]
fn summer_hours_start_on_the_first_of_april() {
    assert_eq!(
        at("2027-03-31T21:59:59+02:00"),
        closes("2027-03-31T22:00:00+02:00")
    );
    assert_eq!(
        at("2027-03-31T22:00:00+02:00"),
        opens("2027-04-01T06:00:00+02:00")
    );
    assert_eq!(
        at("2027-04-01T06:00:00+02:00"),
        closes("2027-04-02T00:00:00+02:00")
    );
}

#[test]
fn winter_hours_start_on_the_first_of_october() {
    assert_eq!(
        at("2026-09-30T23:59:59+02:00"),
        closes("2026-10-01T00:00:00+02:00")
    );
    assert_eq!(
        at("2026-10-01T00:00:00+02:00"),
        opens("2026-10-01T06:00:00+02:00")
    );
    assert_eq!(
        at("2026-10-01T06:00:00+02:00"),
        closes("2026-10-01T22:00:00+02:00")
    );
}

#[test]
fn the_night_the_clocks_go_back_the_park_opens_in_winter_time() {
    for now in [
        "2026-10-24T22:00:00+02:00",
        "2026-10-25T02:30:00+02:00",
        "2026-10-25T02:30:00+01:00",
    ] {
        assert_eq!(at(now), opens("2026-10-25T06:00:00+01:00"), "at {now}");
    }
    assert_eq!(
        at("2026-10-25T06:00:00+01:00"),
        closes("2026-10-25T22:00:00+01:00")
    );
}

#[test]
fn the_night_the_clocks_go_forward_the_park_opens_in_summer_time() {
    for now in ["2027-03-27T22:00:00+01:00", "2027-03-28T01:59:59+01:00"] {
        assert_eq!(at(now), opens("2027-03-28T06:00:00+02:00"), "at {now}");
    }
    assert_eq!(
        at("2027-03-28T06:00:00+02:00"),
        closes("2027-03-28T22:00:00+02:00")
    );
}
