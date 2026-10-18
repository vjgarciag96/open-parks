use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use jiff::{SignedDuration, Timestamp};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use reqwest::Url;
use tokio::sync::watch;

use crate::api::{to_the_second, upstream_failure_status};
use crate::clock::Clock;
use crate::park_status::ParkStatus;
use crate::poller::{Fetched, Latest};
use crate::strings::{Age, Lang, Strings};

const STYLE: &str = include_str!("page/page.css");
const TELEGRAM_LOGO: &str = include_str!("page/telegram_logo.svg");
const FAVICON: &str = include_str!("page/favicon.svg");

const STALE_AFTER: SignedDuration = SignedDuration::from_mins(10);

const SOURCE_URL: &str = "https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es";

#[derive(Debug, Clone)]
pub struct PublicUrl(Url);

impl PublicUrl {
    pub fn new(url: Url) -> Option<PublicUrl> {
        let origin = matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none();
        origin.then_some(PublicUrl(url))
    }

    fn of(&self, path: &str) -> String {
        self.0
            .join(path)
            .expect("an HTTP(S) origin joins any absolute path")
            .into()
    }

    fn alternates(&self) -> impl Iterator<Item = (&'static str, String)> {
        Lang::ALL
            .into_iter()
            .map(|lang| (lang.strings().code, self.of(lang.strings().path)))
            .chain([("x-default", self.of("/"))])
    }
}

struct Site {
    telegram_bot_username: Option<String>,
    public_url: Option<PublicUrl>,
}

#[derive(Clone)]
struct Status {
    latest: watch::Receiver<Latest>,
    clock: Arc<dyn Clock>,
    site: Arc<Site>,
}

pub fn router(
    latest: watch::Receiver<Latest>,
    clock: impl Clock,
    telegram_bot_username: Option<String>,
    public_url: Option<PublicUrl>,
) -> Router {
    Router::new()
        .route(
            "/",
            get(async |headers: HeaderMap| redirect_to_preferred(&headers)),
        )
        .route("/es", get(async |latest| retiro_status(latest, Lang::Es)))
        .route("/en", get(async |latest| retiro_status(latest, Lang::En)))
        .route(
            "/robots.txt",
            get(async |State(status): State<Status>| robots_txt(&status.site)),
        )
        .route(
            "/sitemap.xml",
            get(async |State(status): State<Status>| sitemap_xml(&status.site)),
        )
        .route("/favicon.svg", get(async || favicon()))
        .with_state(Status {
            latest,
            clock: Arc::new(clock),
            site: Arc::new(Site {
                telegram_bot_username,
                public_url,
            }),
        })
}

fn robots_txt(site: &Site) -> String {
    let mut robots = String::from("User-agent: *\nDisallow: /api/\n");
    if let Some(public_url) = &site.public_url {
        robots.push_str(&format!("Sitemap: {}\n", public_url.of("/sitemap.xml")));
    }
    robots
}

fn sitemap_xml(site: &Site) -> Response {
    let Some(public_url) = &site.public_url else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let alternates: String = public_url
        .alternates()
        .map(|(hreflang, href)| {
            format!(r#"<xhtml:link rel="alternate" hreflang="{hreflang}" href="{href}"/>"#)
        })
        .collect();
    let urls: String = Lang::ALL
        .into_iter()
        .map(|lang| {
            let loc = public_url.of(lang.strings().path);
            format!("<url><loc>{loc}</loc>{alternates}</url>")
        })
        .collect();
    let sitemap = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">{urls}</urlset>"#
    );
    ([(header::CONTENT_TYPE, "application/xml")], sitemap).into_response()
}

fn favicon() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        FAVICON,
    )
        .into_response()
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

fn retiro_status(
    State(Status {
        latest,
        clock,
        site,
    }): State<Status>,
    lang: Lang,
) -> Response {
    let strings = lang.strings();
    let other = lang.other().strings();
    match &*latest.borrow() {
        Ok(fetched) => {
            let answer = (strings.headline)(fetched.status.code.verdict());
            render(
                strings,
                other,
                &site,
                answer,
                details(strings, fetched, clock.now()),
            )
            .into_response()
        }
        Err(error) => {
            let (status, _) = upstream_failure_status(error);
            let page = render(strings, other, &site, strings.upstream_failure, html! {});
            (status, page).into_response()
        }
    }
}

fn details(strings: &Strings, fetched: &Fetched, now: Timestamp) -> Markup {
    let elapsed = now.duration_since(fetched.fetched_at);
    html! {
        (status_details(strings, &fetched.status))
        p.updated {
            (strings.updated)
            time datetime=(to_the_second(fetched.fetched_at)) { ((strings.ago)(Age::of(elapsed))) }
        }
        @if elapsed >= STALE_AFTER {
            p.stale { (strings.stale) }
        }
    }
}

fn status_details(strings: &Strings, status: &ParkStatus) -> Markup {
    let mut rows = strings.details(status).peekable();
    html! {
        @if rows.peek().is_some() {
            dl {
                @for (term, description) in rows {
                    dt { (term) }
                    dd { (description) }
                }
            }
        }
    }
}

fn render(
    strings: &Strings,
    other: &Strings,
    site: &Site,
    answer: &str,
    details: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(strings.code) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (strings.title) }
                meta name="description" content=(strings.description);
                @if let Some(public_url) = &site.public_url {
                    link rel="canonical" href=(public_url.of(strings.path));
                    @for (hreflang, href) in public_url.alternates() {
                        link rel="alternate" hreflang=(hreflang) href=(href);
                    }
                    meta property="og:url" content=(public_url.of(strings.path));
                }
                meta property="og:type" content="website";
                meta property="og:title" content={ (strings.question) " " (answer) };
                meta property="og:description" content=(strings.description);
                meta property="og:locale" content=(strings.locale);
                meta property="og:locale:alternate" content=(other.locale);
                link rel="icon" href="/favicon.svg" type="image/svg+xml";
                style { (PreEscaped(STYLE)) }
            }
            body {
                nav {
                    a href=(other.path) hreflang=(other.code) lang=(other.code) {
                        (other.name)
                    }
                }
                main {
                    h1 {
                        span.question { (strings.question) }
                        " "
                        (answer)
                    }
                    (details)
                    table.opening-hours {
                        caption { (strings.opening_hours) }
                        @for (months, hours) in strings.seasons {
                            tr {
                                th scope="row" { (months) }
                                td { (hours) }
                            }
                        }
                    }
                    @if let Some(bot) = &site.telegram_bot_username {
                        p.telegram {
                            a href={ "https://t.me/" (bot) } {
                                (PreEscaped(TELEGRAM_LOGO))
                                (strings.telegram)
                            }
                        }
                    }
                }
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
