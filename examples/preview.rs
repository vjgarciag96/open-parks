//! Renders the status page in every state to `target/preview/`, so it can be
//! looked at in a browser without a running server or a real SIGMA.

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use jiff::{SignedDuration, Timestamp};
use open_parks::page;
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma::{self, FetchError};
use tokio::sync::watch;
use tower::ServiceExt;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sigma");
const OUTPUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/preview");
const DETAILED: &str = "code_6_red_alert_restrictions";
const LANGS: [&str; 2] = ["es", "en"];

struct Preview {
    name: String,
    latest: Latest,
    age: SignedDuration,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let output = Path::new(OUTPUT);
    fs::create_dir_all(output)?;
    let mut index = String::from(
        "<!DOCTYPE html><meta charset=utf-8><title>Page preview</title><h1>Page preview</h1><ul>",
    );
    for Preview { name, latest, age } in previews()? {
        let now = Timestamp::UNIX_EPOCH + age;
        let router = page::router(watch::channel(latest).1, move || now);
        index.push_str(&format!("<li>{name}:"));
        for lang in LANGS {
            let file = format!("{name}.{lang}.html");
            fs::write(output.join(&file), render(router.clone(), lang).await)?;
            index.push_str(&format!(" <a href=\"{file}\">{lang}</a>"));
        }
        index.push_str("</li>");
    }
    index.push_str("</ul>");
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
        let fetched = |age_name: &str, minutes| Preview {
            name: format!("{name}{age_name}"),
            latest: Ok(Fetched {
                status: status.clone(),
                fetched_at: Timestamp::UNIX_EPOCH,
            }),
            age: SignedDuration::from_mins(minutes),
        };
        previews.push(fetched("", 0));
        if name == DETAILED {
            previews.push(fetched(".5_minutes_old", 5));
            previews.push(fetched(".stale", 15));
            previews.push(fetched(".2_days_old", 2 * 24 * 60));
        }
    }
    for (name, error) in [
        ("upstream_error", FetchError::BodyTooLarge),
        ("upstream_timeout", FetchError::Timeout),
    ] {
        previews.push(Preview {
            name: name.to_owned(),
            latest: Err(error),
            age: SignedDuration::ZERO,
        });
    }
    Ok(previews)
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
