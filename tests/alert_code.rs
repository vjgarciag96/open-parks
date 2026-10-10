use open_parks::alert_code::AlertCode;

#[test]
fn undocumented_codes_are_unknown_and_keep_the_raw_value() {
    for raw in [0, 8, -1, i16::MIN, i16::MAX] {
        assert_eq!(
            AlertCode::from(raw),
            AlertCode::Unknown(raw),
            "raw code {raw}"
        );
    }
}

#[test]
fn documented_codes_map_to_their_sigma_meaning() {
    let cases = [
        (1, AlertCode::Open),
        (2, AlertCode::Incidents),
        (3, AlertCode::YellowAlert),
        (4, AlertCode::OrangeAlert),
        (5, AlertCode::RedAlertClosure),
        (6, AlertCode::RedAlertRestrictions),
        (7, AlertCode::Closed),
    ];
    for (raw, expected) in cases {
        assert_eq!(AlertCode::from(raw), expected, "raw code {raw}");
    }
}
