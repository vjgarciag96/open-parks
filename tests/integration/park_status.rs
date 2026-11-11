use jiff::Timestamp;
use open_parks::park::status::{AlertCode, ParkStatus, Verdict};

#[test]
fn open_park_is_open() {
    let verdict = AlertCode::Open.verdict();

    assert_eq!(verdict, Verdict::Open);
}

#[test]
fn incidents_and_yellow_alert_leave_the_park_open_with_restrictions() {
    for code in [AlertCode::Incidents, AlertCode::YellowAlert] {
        let verdict = code.verdict();

        assert_eq!(verdict, Verdict::OpenWithRestrictions, "{code:?}");
    }
}

#[test]
fn orange_alert_leaves_the_park_open_but_advises_against_staying() {
    let verdict = AlertCode::OrangeAlert.verdict();

    assert_eq!(verdict, Verdict::OpenStayNotAdvised);
}

#[test]
fn red_alert_closure_forecasts_a_closure() {
    let verdict = AlertCode::RedAlertClosure.verdict();

    assert_eq!(verdict, Verdict::ClosureForecast);
}

#[test]
fn red_alert_restrictions_forecasts_a_closure() {
    let verdict = AlertCode::RedAlertRestrictions.verdict();

    assert_eq!(verdict, Verdict::ClosureForecast);
}

#[test]
fn closed_park_is_closed() {
    let verdict = AlertCode::Closed.verdict();

    assert_eq!(verdict, Verdict::Closed);
}

#[test]
fn unknown_code_has_an_unknown_verdict() {
    let verdict = AlertCode::Unknown(8).verdict();

    assert_eq!(verdict, Verdict::Unknown);
}

fn red_alert(code: AlertCode, dates: Option<&str>, hours: Option<&str>) -> ParkStatus {
    ParkStatus {
        code,
        dates: dates.map(Into::into),
        hours: hours.map(Into::into),
        reopening: None,
        notes: None,
    }
}

// Every case falls in November, when Madrid is an hour ahead of UTC.
fn at(madrid_time: &str) -> Timestamp {
    format!("{madrid_time}:00+01:00").parse().unwrap()
}

#[test]
fn a_forecast_closure_is_a_closure_while_its_window_is_under_way() {
    for code in [AlertCode::RedAlertClosure, AlertCode::RedAlertRestrictions] {
        for hours in ["12:00-24:00", "12:00 a 24:00"] {
            let status = red_alert(code, Some("11/11/2026"), Some(hours));

            for now in ["2026-11-11T12:00", "2026-11-11T20:06", "2026-11-11T23:59"] {
                assert_eq!(
                    status.verdict_at(at(now)),
                    Verdict::Closed,
                    "{code:?} {hours} at {now}"
                );
            }
        }
    }
}

#[test]
fn a_forecast_closure_is_still_a_forecast_outside_its_window() {
    let status = red_alert(
        AlertCode::RedAlertRestrictions,
        Some("11/11/2026"),
        Some("03:00 a 12:00"),
    );

    for now in [
        "2026-11-10T12:00",
        "2026-11-11T02:59",
        "2026-11-11T12:00",
        "2026-11-12T03:00",
    ] {
        assert_eq!(
            status.verdict_at(at(now)),
            Verdict::ClosureForecast,
            "at {now}"
        );
    }
}

#[test]
fn a_forecast_closure_whose_window_cant_be_read_stays_a_forecast() {
    let cases = [
        (None, Some("12:00-24:00")),
        (Some("11/11/2026"), None),
        (Some("11/11/2026 - 12/11/2026"), Some("12:00-24:00")),
        (Some("11/11/2026"), Some("Todo el día")),
        (Some("11/11/2026"), Some("20:00 a 12:00")),
        (Some("31/11/2026"), Some("12:00-24:00")),
    ];
    for (dates, hours) in cases {
        let status = red_alert(AlertCode::RedAlertRestrictions, dates, hours);

        assert_eq!(
            status.verdict_at(at("2026-11-11T20:06")),
            Verdict::ClosureForecast,
            "{dates:?} {hours:?}"
        );
    }
}

#[test]
fn a_window_does_not_change_any_other_verdict() {
    for code in [
        AlertCode::Open,
        AlertCode::Incidents,
        AlertCode::YellowAlert,
        AlertCode::OrangeAlert,
        AlertCode::Closed,
        AlertCode::Unknown(8),
    ] {
        let status = red_alert(code, Some("11/11/2026"), Some("12:00-24:00"));

        assert_eq!(
            status.verdict_at(at("2026-11-11T20:06")),
            code.verdict(),
            "{code:?}"
        );
    }
}
