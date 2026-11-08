use std::collections::BTreeSet;
use std::fs;

use open_parks::park::forecast::Weather;
use open_parks::web::page::weather_icon;

const ICONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/weather");

/// Every WMO code that is weather, with its icon by day and by night.
fn icons() -> Vec<(u8, &'static str, &'static str)> {
    (0..=u8::MAX)
        .filter_map(|code| {
            let weather = Weather::from_wmo(code)?;
            Some((
                code,
                weather_icon::svg(weather, true),
                weather_icon::svg(weather, false),
            ))
        })
        .collect()
}

#[test]
fn every_icon_file_is_shown_for_some_weather() {
    let shown: BTreeSet<&str> = icons()
        .into_iter()
        .flat_map(|(_, day, night)| [day, night])
        .collect();

    let files: BTreeSet<String> = fs::read_dir(ICONS)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .map(|path| fs::read_to_string(path).unwrap())
        .collect();

    assert_eq!(files.len(), 36, "icon files");
    assert_eq!(
        shown,
        files.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        "icons shown and icon files"
    );
}

#[test]
fn only_clear_and_partly_cloudy_skies_and_showers_have_a_night_icon() {
    let with_night: Vec<u8> = icons()
        .into_iter()
        .filter(|(_, day, night)| day != night)
        .map(|(code, _, _)| code)
        .collect();

    assert_eq!(with_night, [0, 1, 2, 80, 81, 82, 85, 86]);
}

#[test]
fn every_icon_is_hidden_from_screen_readers() {
    let hidden: Vec<bool> = icons()
        .into_iter()
        .flat_map(|(_, day, night)| [day, night])
        .map(|svg| svg.starts_with("<svg ") && svg.contains(r#" aria-hidden="true""#))
        .collect();

    assert!(hidden.iter().all(|&hidden| hidden), "{hidden:?}");
}
