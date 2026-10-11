use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use tokio::sync::watch;

use crate::api::upstream_failure_status;
use crate::poller::Latest;

mod strings;
use strings::{Lang, Strings};

const STYLE: &str = include_str!("page.css");

const SOURCE_URL: &str = "https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es";

pub fn router(latest: watch::Receiver<Latest>) -> Router {
    Router::new()
        .route(
            "/",
            get(async |headers: HeaderMap| redirect_to_preferred(&headers)),
        )
        .route("/es", get(async |latest| retiro_status(latest, Lang::Es)))
        .route("/en", get(async |latest| retiro_status(latest, Lang::En)))
        .with_state(latest)
}

fn redirect_to_preferred(headers: &HeaderMap) -> Response {
    let lang = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(preferred)
        .unwrap_or(Lang::Es);
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, lang.strings().path),
            (header::VARY, "accept-language"),
        ],
    )
        .into_response()
}

fn preferred(accept_language: &str) -> Option<Lang> {
    accept_language
        .split(',')
        .filter_map(Preference::parse)
        .filter(|preference| preference.quality > 0.0)
        .reduce(|best, next| {
            if next.quality > best.quality {
                next
            } else {
                best
            }
        })
        .map(|preference| preference.lang)
}

struct Preference {
    lang: Lang,
    quality: f32,
}

impl Preference {
    fn parse(range: &str) -> Option<Preference> {
        let (tag, params) = range.split_once(';').unwrap_or((range, ""));
        Some(Preference {
            lang: Lang::from_tag(tag)?,
            quality: quality(params)?,
        })
    }
}

fn quality(params: &str) -> Option<f32> {
    let q = params.split(';').find_map(|param| {
        let (name, value) = param.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("q")
            .then_some(value.trim())
    });
    match q {
        None => Some(1.0),
        Some(q) => q.parse().ok().filter(|q| (0.0..=1.0).contains(q)),
    }
}

fn retiro_status(State(latest): State<watch::Receiver<Latest>>, lang: Lang) -> Response {
    let strings = lang.strings();
    let other = lang.other().strings();
    match &*latest.borrow() {
        Ok(fetched) => {
            let answer = (strings.headline)(fetched.status.code.verdict());
            render(strings, other, answer).into_response()
        }
        Err(error) => {
            let (status, _) = upstream_failure_status(error);
            (status, render(strings, other, strings.upstream_failure)).into_response()
        }
    }
}

fn render(strings: &Strings, other: &Strings, answer: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(strings.code) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (strings.question) }
                style { (PreEscaped(STYLE)) }
            }
            body {
                nav {
                    a href=(other.path) hreflang=(other.code) lang=(other.code) {
                        (other.name)
                    }
                }
                p { (strings.question) }
                h1 { (answer) }
                footer {
                    (strings.source_lead)
                    a href=(SOURCE_URL) { (strings.source_link) }
                    " ("
                    a href=(strings.licence_url) { "CC BY 4.0" }
                    "). "
                    (strings.disclaimer)
                }
            }
        }
    }
}
