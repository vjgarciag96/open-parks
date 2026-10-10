use open_parks::park_status::{AlertCode, ParkStatus, ReopeningForecast};
use open_parks::sigma::{self, DecodeError};

mod common;
use common::fixture;

#[test]
fn open_park_decodes_with_its_date_and_no_other_details() {
    let status = sigma::decode(&fixture("code_1_open.json")).unwrap();

    assert_eq!(
        status,
        ParkStatus {
            code: AlertCode::Open,
            dates: Some("10/10/2026".to_owned()),
            hours: None,
            reopening: None,
            notes: None,
        }
    );
}

#[test]
fn red_alert_closure_decodes_reopening_after_inspections() {
    let status = sigma::decode(&fixture("code_5_red_alert_closure.json")).unwrap();

    assert_eq!(
        status,
        ParkStatus {
            code: AlertCode::RedAlertClosure,
            dates: Some("12/10/2026".to_owned()),
            hours: None,
            reopening: Some(ReopeningForecast::AfterInspections),
            notes: None,
        }
    );
}

#[test]
fn closed_park_decodes_its_notes_and_a_see_notes_reopening() {
    let status = sigma::decode(&fixture("code_7_closed.json")).unwrap();

    assert_eq!(
        status,
        ParkStatus {
            code: AlertCode::Closed,
            dates: Some("13/10/2026".to_owned()),
            hours: None,
            reopening: Some(ReopeningForecast::SeeNotes),
            notes: Some(
                "El parque permanecerá cerrado hasta la retirada de las ramas caídas.".to_owned()
            ),
        }
    );
}

#[test]
fn alerts_with_a_time_window_decode_their_dates_hours_and_notes() {
    let cases = [
        (
            "code_2_incidents.json",
            ParkStatus {
                code: AlertCode::Incidents,
                dates: Some("10/10/2026".to_owned()),
                hours: Some("de 08:00 a 14:00 h".to_owned()),
                reopening: None,
                notes: Some("Cerrada la Rosaleda por trabajos de poda.".to_owned()),
            },
        ),
        (
            "code_3_yellow_alert.json",
            ParkStatus {
                code: AlertCode::YellowAlert,
                dates: Some("11/10/2026".to_owned()),
                hours: Some("de 12:00 a 20:00 h".to_owned()),
                reopening: None,
                notes: None,
            },
        ),
        (
            "code_4_orange_alert.json",
            ParkStatus {
                code: AlertCode::OrangeAlert,
                dates: Some("11/10/2026 - 12/10/2026".to_owned()),
                hours: Some("de 10:00 a 22:00 h".to_owned()),
                reopening: None,
                notes: None,
            },
        ),
        (
            "code_6_red_alert_restrictions.json",
            ParkStatus {
                code: AlertCode::RedAlertRestrictions,
                dates: Some("12/10/2026".to_owned()),
                hours: Some("de 14:00 a 20:00 h".to_owned()),
                reopening: None,
                notes: Some(
                    "Cerradas las zonas infantiles, deportivas y de mayores. \
                     Suspendidos los eventos al aire libre."
                        .to_owned(),
                ),
            },
        ),
    ];
    for (name, expected) in cases {
        assert_eq!(
            sigma::decode(&fixture(name)).unwrap(),
            expected,
            "fixture {name}"
        );
    }
}

#[test]
fn a_response_without_features_is_an_error() {
    let result = sigma::decode(&fixture("no_features.json"));

    assert!(
        matches!(result, Err(DecodeError::NoFeatures)),
        "got {result:?}"
    );
}

#[test]
fn a_response_with_several_features_is_an_error_rather_than_a_guess() {
    let result = sigma::decode(&fixture("multiple_features.json"));

    assert!(
        matches!(result, Err(DecodeError::MultipleFeatures(2))),
        "got {result:?}"
    );
}

#[test]
fn malformed_json_is_an_error() {
    let result = sigma::decode(&fixture("malformed.json"));

    assert!(
        matches!(result, Err(DecodeError::Malformed(_))),
        "got {result:?}"
    );
}

#[test]
fn an_error_body_sent_with_status_200_is_an_upstream_error() {
    let result = sigma::decode(&fixture("error_body.json"));

    assert!(
        matches!(
            &result,
            Err(DecodeError::Upstream { code: 400, message })
                if message == "Unable to complete operation."
        ),
        "got {result:?}"
    );
}

#[test]
fn an_error_body_without_a_message_is_malformed_and_names_the_field() {
    let result = sigma::decode(&fixture("error_body_without_message.json"));

    let Err(error @ DecodeError::Malformed(_)) = result else {
        panic!("expected Malformed, got {result:?}");
    };
    assert!(
        error.to_string().contains("message"),
        "error should name the field: {error}"
    );
}

#[test]
fn a_missing_alert_code_is_malformed_and_names_the_field() {
    let result = sigma::decode(&fixture("null_alert_code.json"));

    let Err(error @ DecodeError::Malformed(_)) = result else {
        panic!("expected Malformed, got {result:?}");
    };
    assert!(
        error.to_string().contains("ALERTA_DESCRIPCION"),
        "error should name the field: {error}"
    );
}

#[test]
fn trailing_data_after_the_json_is_malformed() {
    let result = sigma::decode(&fixture("trailing_garbage.json"));

    assert!(
        matches!(result, Err(DecodeError::Malformed(_))),
        "got {result:?}"
    );
}

#[test]
fn blank_free_text_fields_decode_as_absent() {
    let status = sigma::decode(&fixture("blank_strings.json")).unwrap();

    assert_eq!(
        status,
        ParkStatus {
            code: AlertCode::Open,
            dates: None,
            hours: None,
            reopening: None,
            notes: None,
        }
    );
}

#[test]
fn undocumented_codes_decode_as_unknown_with_the_raw_value() {
    let status = sigma::decode(&fixture("unknown_codes.json")).unwrap();

    assert_eq!(
        status,
        ParkStatus {
            code: AlertCode::Unknown(8),
            dates: Some("10/10/2026".to_owned()),
            hours: None,
            reopening: Some(ReopeningForecast::Unknown(3)),
            notes: None,
        }
    );
}
