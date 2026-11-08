//! Renders the status page in every state to `target/preview/`, so it can be
//! looked at in a browser without a running server or a real SIGMA.

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use jiff::{SignedDuration, Timestamp};
use open_parks::open_meteo;
use open_parks::park::forecast::Weather;
use open_parks::sigma::{self, FetchError};
use open_parks::sigma::{Fetched, Latest};
use open_parks::web::page::{self, weather_icon};
use tokio::sync::watch;
use tower::ServiceExt;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sigma");
const FORECAST: &str = include_str!("../tests/fixtures/open_meteo/rain_chances.json");
const STYLE: &str = include_str!("../assets/page.css");
const OUTPUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/preview");
const DETAILED: &str = "code_6_red_alert_restrictions";
const LANGS: [&str; 2] = ["es", "en"];
const TELEGRAM_BOT: &str = "retiro_abierto_bot";
const SCHEMES: [&str; 2] = ["light", "dark"];
const SCHEMES_HEAD: &str = "<!DOCTYPE html><meta charset=utf-8><style>body{margin:1rem;font-family:system-ui;background:#888}.frames{display:flex;flex-wrap:wrap;gap:12px}iframe{border:0}.phone{width:390px;height:844px;border-radius:12px}.desktop{width:640px;height:400px;overflow:hidden;border-radius:12px}.desktop iframe{width:1280px;height:800px;transform:scale(0.5);transform-origin:0 0}</style>";
// Inside the forecast fixture's two days.
const DAY: &str = "2026-11-02T09:30:00Z";
const EVENING: &str = "2026-11-02T19:30:00Z";
const NIGHT: &str = "2026-11-02T21:30:00Z";
const OLD_FORECAST: &str = "2026-11-02T04:30:00Z";

struct Preview {
    name: String,
    latest: Latest,
    forecast: open_meteo::Latest,
    fetched_at: Timestamp,
    age: SignedDuration,
    telegram_bot_username: Option<&'static str>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let output = Path::new(OUTPUT);
    fs::create_dir_all(output)?;
    let mut index = String::from(
        "<!DOCTYPE html><meta charset=utf-8><title>Page preview</title><h1>Page preview</h1><ul>",
    );
    for Preview {
        name,
        latest,
        forecast,
        fetched_at,
        age,
        telegram_bot_username,
    } in previews()?
    {
        let now = fetched_at + age;
        let router = page::router(
            watch::channel(latest).1,
            watch::channel(forecast).1,
            move || now,
            telegram_bot_username.map(str::to_owned),
            None,
        );
        index.push_str(&format!("<li>{name}:"));
        let mut phones = String::new();
        let mut desktops = String::new();
        for lang in LANGS {
            let file = format!("{name}.{lang}.html");
            fs::write(output.join(&file), render(router.clone(), lang).await)?;
            index.push_str(&format!(" <a href=\"{file}\">{lang}</a>"));
            for scheme in SCHEMES {
                let frame = format!(
                    "<iframe src=\"{file}\" style=\"color-scheme: {scheme}\" title=\"{lang} {scheme}\"></iframe>"
                );
                phones.push_str(&frame.replacen("<iframe", "<iframe class=\"phone\"", 1));
                desktops.push_str(&format!("<div class=\"desktop\">{frame}</div>"));
            }
        }
        let schemes = format!("{name}.html");
        fs::write(
            output.join(&schemes),
            format!(
                "{SCHEMES_HEAD}<title>{name}</title><h1>{name}</h1>\
                 <h2>Desktop</h2><div class=\"frames\">{desktops}</div>\
                 <h2>Phone</h2><div class=\"frames\">{phones}</div>"
            ),
        )?;
        index.push_str(&format!(" <a href=\"{schemes}\">light &amp; dark</a></li>"));
    }
    index.push_str(&format!("<li>{}</li></ul>", weather_icons(output)?));
    let index_path = output.join("index.html");
    fs::write(&index_path, index)?;
    println!("open {}", index_path.display());
    Ok(())
}

fn previews() -> std::io::Result<Vec<Preview>> {
    let mut previews = Vec::new();
    for (name, json) in fixtures()? {
        // Fixtures that don't decode are for the decoder's failure paths.
        let Ok(status) = sigma::decode(&json) else {
            continue;
        };
        let fetched = |variant: &str, at: &str, minutes, telegram_bot_username| {
            let fetched_at = at.parse().unwrap();
            Preview {
                name: format!("{name}{variant}"),
                latest: Ok(Fetched {
                    status: status.clone(),
                    fetched_at,
                }),
                forecast: forecast_fetched_at(at),
                fetched_at,
                age: SignedDuration::from_mins(minutes),
                telegram_bot_username,
            }
        };
        previews.push(fetched("", DAY, 0, None));
        previews.push(fetched(".night", NIGHT, 0, None));
        if name == DETAILED {
            previews.push(fetched(".evening", EVENING, 0, None));
            previews.push(fetched(".5_minutes_old", DAY, 5, None));
            previews.push(fetched(".stale", DAY, 15, None));
            previews.push(fetched(".2_days_old", DAY, 2 * 24 * 60, None));
            previews.push(fetched(".telegram_bot", DAY, 0, Some(TELEGRAM_BOT)));
            previews.push(Preview {
                forecast: forecast_fetched_at(OLD_FORECAST),
                ..fetched(".old_forecast", DAY, 0, None)
            });
            previews.push(Preview {
                forecast: None,
                ..fetched(".no_forecast", DAY, 0, None)
            });
            previews.push(Preview {
                forecast: missing_values_fetched_at(DAY),
                ..fetched(".missing_values", DAY, 0, None)
            });
            previews.push(Preview {
                forecast: risky_values_fetched_at(DAY),
                ..fetched(".risky_values", DAY, 0, None)
            });
        }
    }
    for (name, error, at, telegram_bot_username) in [
        ("upstream_error", FetchError::BodyTooLarge, DAY, None),
        (
            "upstream_error.night",
            FetchError::BodyTooLarge,
            NIGHT,
            None,
        ),
        ("upstream_timeout", FetchError::Timeout, DAY, None),
        (
            "upstream_timeout.telegram_bot",
            FetchError::Timeout,
            DAY,
            Some(TELEGRAM_BOT),
        ),
    ] {
        previews.push(Preview {
            name: name.to_owned(),
            latest: Err(error),
            forecast: forecast_fetched_at(at),
            fetched_at: at.parse().unwrap(),
            age: SignedDuration::ZERO,
            telegram_bot_username,
        });
    }
    Ok(previews)
}

fn forecast_fetched_at(at: &str) -> open_meteo::Latest {
    Some(open_meteo::Fetched {
        forecast: open_meteo::decode(FORECAST).unwrap(),
        fetched_at: at.parse().unwrap(),
    })
}

/// The forecast with one value left out in each hour from 11:00 to 16:00,
/// or at 13:00 a weather code with no icon, as Open-Meteo might send it.
fn missing_values_fetched_at(at: &str) -> open_meteo::Latest {
    let mut latest = forecast_fetched_at(at);
    for hour in &mut latest.as_mut().unwrap().forecast.hours {
        match (hour.at.day(), hour.at.hour()) {
            (2, 11) => hour.rain_chance = None,
            (2, 12) => hour.gusts = None,
            (2, 13) => hour.weather_code = Some(4),
            (2, 14) => hour.temperature = None,
            (2, 15) => hour.weather_code = None,
            (2, 16) => hour.is_day = None,
            _ => {}
        }
    }
    latest
}

/// The forecast with every value risky in the first hour, in the shaded "Now"
/// column, then each cut-off in one hour from 11:00 to 14:00.
fn risky_values_fetched_at(at: &str) -> open_meteo::Latest {
    let mut latest = forecast_fetched_at(at);
    for hour in &mut latest.as_mut().unwrap().forecast.hours {
        match (hour.at.day(), hour.at.hour()) {
            (2, 10) => {
                hour.temperature = Some(35.0);
                hour.rain_chance = Some(60);
                hour.gusts = Some(40.0);
            }
            (2, 11) => hour.temperature = Some(35.0),
            (2, 12) => hour.temperature = Some(0.0),
            (2, 13) => hour.rain_chance = Some(60),
            (2, 14) => hour.gusts = Some(40.0),
            _ => {}
        }
    }
    latest
}

/// Writes every weather icon, by day and by night, in light and dark, and
/// returns the index's link to them.
fn weather_icons(output: &Path) -> std::io::Result<String> {
    let mut sheet = format!(
        "<!DOCTYPE html><meta charset=utf-8><style>{STYLE}\
         .sheet{{align-self:stretch;display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:12px;margin:1rem}}\
         .sheet div{{display:flex;flex-direction:column;align-items:center;gap:6px;padding:12px;border-radius:12px;background:var(--card);font-size:.75rem;color:var(--muted);text-align:center}}\
         .sheet .weather{{width:56px;height:56px}}</style><div class=sheet>"
    );
    for code in 0..=u8::MAX {
        let Some(weather) = Weather::from_wmo(code) else {
            continue;
        };
        let (day, night) = (
            weather_icon::svg(weather, true),
            weather_icon::svg(weather, false),
        );
        sheet.push_str(&format!("<div>{day}{day}{code} {weather:?}</div>"));
        if night != day {
            sheet.push_str(&format!(
                "<div>{night}{night}{code} {weather:?} at night</div>"
            ));
        }
    }
    sheet.push_str("</div>");
    fs::write(output.join("weather_icons.sheet.html"), sheet)?;
    let frames: String = SCHEMES
        .iter()
        .map(|scheme| {
            format!(
                "<iframe src=\"weather_icons.sheet.html\" style=\"color-scheme: {scheme}; width: 100%; height: 1300px\" title=\"{scheme}\"></iframe>"
            )
        })
        .collect();
    fs::write(
        output.join("weather_icons.html"),
        format!("{SCHEMES_HEAD}<title>Weather icons</title><h1>Weather icons</h1>{frames}"),
    )?;
    Ok("Weather icons: <a href=\"weather_icons.html\">light &amp; dark</a>".to_owned())
}

fn fixtures() -> std::io::Result<Vec<(String, String)>> {
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(FIXTURES)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            fixtures.push((name, fs::read_to_string(&path)?));
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

async fn render(router: Router, lang: &str) -> Vec<u8> {
    let request = Request::get(format!("/{lang}"))
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}
