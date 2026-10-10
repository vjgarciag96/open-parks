use open_parks::park_status::{AlertCode, Verdict};

#[test]
fn open_park_is_open() {
    let verdict = AlertCode::Open.verdict();

    assert_eq!(verdict, Verdict::Open);
}

#[test]
fn incidents_and_restricting_alerts_leave_the_park_open_with_restrictions() {
    for code in [
        AlertCode::Incidents,
        AlertCode::YellowAlert,
        AlertCode::RedAlertRestrictions,
    ] {
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
fn closed_park_is_closed() {
    let verdict = AlertCode::Closed.verdict();

    assert_eq!(verdict, Verdict::Closed);
}

#[test]
fn unknown_code_has_an_unknown_verdict() {
    let verdict = AlertCode::Unknown(8).verdict();

    assert_eq!(verdict, Verdict::Unknown);
}
