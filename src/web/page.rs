use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{MethodRouter, get};
use jiff::civil::{Date, Time};
use jiff::{SignedDuration, Timestamp};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use tokio::sync::watch;

use crate::core::clock::Clock;
use crate::i18n::Lang;
use crate::i18n::{Age, Strings};
use crate::open_meteo;
use crate::park::headline::{Alerts, Headline};
use crate::park::schedule;
use crate::park::status::{AlertClosure, ParkStatus, Verdict};
use crate::sigma::{Fetched, Latest};
use crate::web::public_url::PublicUrl;
use crate::web::structured_data;

mod llms_txt;
mod next_hours;
mod og_card;
pub mod weather_icon;

const STYLE: &str = include_str!("../../assets/page.css");
const TELEGRAM_LOGO: &str = include_str!("../../assets/telegram_logo.svg");
const FAVICON: &str = include_str!("../../assets/favicon.svg");

const STALE_AFTER: SignedDuration = SignedDuration::from_mins(10);

const WIDE_AFTER: usize = 32;
const LONG_ANSWER_AFTER: usize = 12;

const FORECAST_SOURCE_URL: &str = "https://open-meteo.com/";
const SOURCE_URL: &str = "https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es";

fn alternates(public_url: &PublicUrl) -> impl Iterator<Item = (&'static str, String)> {
    Lang::ALL
        .into_iter()
        .map(|lang| (lang.code(), public_url.of(lang.path())))
        .chain([("x-default", public_url.of(Lang::Es.path()))])
}

struct Site {
    telegram_bot_username: Option<String>,
    public_url: Option<PublicUrl>,
}

#[derive(Clone)]
struct PageState {
    latest: watch::Receiver<Latest>,
    forecast: watch::Receiver<open_meteo::Latest>,
    site: Arc<Site>,
}

pub fn router(
    latest: watch::Receiver<Latest>,
    forecast: watch::Receiver<open_meteo::Latest>,
    clock: impl Clock,
    telegram_bot_username: Option<String>,
    public_url: Option<PublicUrl>,
) -> Router {
    let clock: Arc<dyn Clock> = Arc::new(clock);
    let status_in = |lang: Lang| {
        let clock = clock.clone();
        get(async move |State(state): State<PageState>| retiro_status(&state, lang, clock.now()))
    };
    Router::new()
        .route("/", status_in(Lang::Es))
        // Where the Spanish page was before it moved to the root.
        .route("/es", redirect_to(Lang::Es.path()))
        .route("/es/", redirect_to(Lang::Es.path()))
        .route("/en", status_in(Lang::En))
        // A trailing slash is a common way to type either address.
        .route("/en/", redirect_to(Lang::En.path()))
        .route(
            "/robots.txt",
            get(async |State(state): State<PageState>| robots_txt(&state.site)),
        )
        .route(
            "/sitemap.xml",
            get(async |State(state): State<PageState>| sitemap_xml(&state.site)),
        )
        .route(
            "/llms.txt",
            get(async |State(state): State<PageState>| llms_txt::response(&state.site)),
        )
        .route("/favicon.svg", get(async || favicon()))
        .route(
            "/og/{file}",
            get(async |Path(file): Path<String>| og_card::response(&file)),
        )
        .with_state(PageState {
            latest,
            forecast,
            site: Arc::new(Site {
                telegram_bot_username,
                public_url,
            }),
        })
}

/// A permanent redirect to `path`, keeping the query string, which may carry
/// a campaign's tags.
fn redirect_to(path: &'static str) -> MethodRouter<PageState> {
    get(async move |RawQuery(query): RawQuery| match query {
        Some(query) => Redirect::permanent(&format!("{path}?{query}")),
        None => Redirect::permanent(path),
    })
}

fn robots_txt(site: &Site) -> String {
    // The API is open too: it's the most exact answer an agent can read.
    // Search engines see its noindex instead.
    let mut robots = String::from("User-agent: *\nAllow: /\n");
    if let Some(public_url) = &site.public_url {
        robots.push_str(&format!("Sitemap: {}\n", public_url.of("/sitemap.xml")));
    }
    robots
}

fn sitemap_xml(site: &Site) -> Response {
    let Some(public_url) = &site.public_url else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let alternates: String = alternates(public_url)
        .map(|(hreflang, href)| {
            format!(r#"<xhtml:link rel="alternate" hreflang="{hreflang}" href="{href}"/>"#)
        })
        .collect();
    let urls: String = Lang::ALL
        .into_iter()
        .map(|lang| {
            let loc = public_url.of(lang.path());
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

fn retiro_status(
    PageState {
        latest,
        forecast,
        site,
    }: &PageState,
    lang: Lang,
    now: Timestamp,
) -> Response {
    let strings = lang.strings();
    let schedule = schedule::retiro(now);
    let next_hours = next_hours::card(strings, &forecast.borrow(), now);
    let latest = latest.borrow();
    let headline = Headline::at(
        latest.as_ref().ok().map(|fetched| &fetched.status),
        &schedule,
        now,
    );
    match &*latest {
        Ok(fetched) => {
            let checked = Checked::of(fetched, now);
            let details = details(strings, fetched, &checked);
            render(
                lang,
                site,
                now,
                &headline,
                Freshness::Checked(checked),
                details,
                next_hours,
            )
            .into_response()
        }
        Err(error) => {
            let page = render(
                lang,
                site,
                now,
                &headline,
                Freshness::Unchecked,
                html! {},
                next_hours,
            );
            (error.gateway_status(), page).into_response()
        }
    }
}

/// How recent the answer is, said under it.
enum Freshness {
    Unchecked,
    Checked(Checked),
}

impl Freshness {
    fn stale(&self) -> bool {
        matches!(self, Freshness::Checked(checked) if checked.stale())
    }
}

/// When the status was fetched, in Madrid time.
struct Checked {
    time: Time,
    /// The day, when it isn't today.
    earlier_day: Option<Date>,
    elapsed: SignedDuration,
}

impl Checked {
    fn of(fetched: &Fetched, now: Timestamp) -> Checked {
        let at = fetched.fetched_at.to_zoned(schedule::madrid());
        let today = now.to_zoned(schedule::madrid()).date();
        Checked {
            time: at.time(),
            earlier_day: (at.date() != today).then_some(at.date()),
            elapsed: now.duration_since(fetched.fetched_at),
        }
    }

    fn stale(&self) -> bool {
        self.elapsed >= STALE_AFTER
    }

    fn line(&self, strings: &Strings) -> String {
        match self.earlier_day {
            Some(day) => (strings.checked_on)(day, self.time),
            None => (strings.checked_at)(self.time),
        }
    }
}

struct Tone {
    class: &'static str,
    icon: &'static str,
}

fn tone(alerts: Alerts) -> Tone {
    let (class, icon) = match alerts {
        Alerts::Checked(Verdict::Open) => ("ok", "✓"),
        Alerts::Checked(Verdict::OpenWithRestrictions | Verdict::OpenStayNotAdvised) => {
            ("warn", "!")
        }
        Alerts::Checked(Verdict::ClosureForecast | Verdict::Closed) => ("danger", "!"),
        Alerts::Checked(Verdict::Unknown) | Alerts::Unchecked => ("neutral", "?"),
    };
    Tone { class, icon }
}

fn details(strings: &Strings, fetched: &Fetched, checked: &Checked) -> Markup {
    let fetched_at = fetched.fetched_at_to_the_second();
    let in_madrid = fetched_at.to_zoned(schedule::madrid());
    html! {
        (status_details(strings, &fetched.status))
        p.updated.stale[checked.stale()] {
            span.dot aria-hidden="true" {}
            span {
                (strings.updated)
                time datetime=(fetched_at) { ((strings.updated_at)(in_madrid.date(), in_madrid.time())) }
                ", "
                span.ago { ((strings.ago)(Age::of(checked.elapsed))) }
            }
        }
    }
}

fn status_details(strings: &Strings, status: &ParkStatus) -> Markup {
    let mut rows = strings.details(status).peekable();
    html! {
        @if rows.peek().is_some() {
            dl {
                @for (term, description) in rows {
                    div.row.wide[description.chars().count() > WIDE_AFTER] {
                        dt { (term) }
                        dd { (description) }
                    }
                }
            }
        }
    }
}

/// What the answer leaves to say of an alert's closure, a line each.
fn closure_lines(strings: &Strings, headline: &Headline) -> Vec<String> {
    match headline.closure {
        Some(closure) if closure.under_way() => {
            let end = closure.end().and_then(|end| strings.closure_end(end));
            [Some(strings.closure_cause.to_owned()), end]
                .into_iter()
                .flatten()
                .collect()
        }
        _ => headline
            .expected_closure_window()
            .map(|window| (strings.closure_expected_between)(window.from, window.to))
            .into_iter()
            .collect(),
    }
}

fn render(
    lang: Lang,
    site: &Site,
    now: Timestamp,
    headline: &Headline,
    freshness: Freshness,
    details: Markup,
    next_hours: Markup,
) -> Markup {
    let strings = lang.strings();
    let tone = tone(headline.alerts);
    let no_closure_hours = headline.closure.is_some_and(AlertClosure::hours_missing);
    let answer = strings.answer(headline.answer);
    let preview_title = format!("{} {}", strings.question, strings.answer(headline.preview));
    html! {
        (DOCTYPE)
        html lang=(lang.code()) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                // The theme colours are page.css's backgrounds.
                meta name="color-scheme" content="light dark";
                meta name="theme-color" content="#f5f5f7" media="(prefers-color-scheme: light)";
                meta name="theme-color" content="#000000" media="(prefers-color-scheme: dark)";
                title { (strings.title) }
                meta name="description" content=(strings.description);
                @if let Some(public_url) = &site.public_url {
                    link rel="canonical" href=(public_url.of(lang.path()));
                    @for (hreflang, href) in alternates(public_url) {
                        link rel="alternate" hreflang=(hreflang) href=(href);
                    }
                    meta property="og:url" content=(public_url.of(lang.path()));
                    meta property="og:image" content=(public_url.of(&og_card::path(headline, lang)));
                    meta property="og:image:width" content=(og_card::WIDTH);
                    meta property="og:image:height" content=(og_card::HEIGHT);
                    meta property="og:image:type" content="image/png";
                    meta property="og:image:alt" content=(preview_title);
                    meta name="twitter:card" content="summary_large_image";
                }
                meta property="og:type" content="website";
                meta property="og:site_name" content=(structured_data::SITE_NAME);
                meta property="og:title" content=(preview_title);
                meta property="og:description" content=(strings.description);
                meta property="og:locale" content=(lang.locale());
                meta property="og:locale:alternate" content=(lang.other().locale());
                link rel="icon" href="/favicon.svg" type="image/svg+xml";
                script type="application/ld+json" {
                    (PreEscaped(structured_data::park(lang, site.public_url.as_ref(), now)))
                }
                @if let Some(public_url) = &site.public_url {
                    script type="application/ld+json" {
                        (PreEscaped(structured_data::website(public_url)))
                    }
                }
                style { (PreEscaped(STYLE)) }
            }
            body {
                // Google quotes page text over the description otherwise, and its copy
                // of the live status is out of date by the time anyone reads it.
                div data-nosnippet {
                    nav {
                        @for language in Lang::ALL {
                            @if language == lang {
                                span aria-current="page" { (language.name()) }
                            } @else {
                                a href=(language.path()) hreflang=(language.code()) lang=(language.code()) {
                                    (language.name())
                                }
                            }
                        }
                    }
                    main {
                        @if freshness.stale() {
                            p.last-known { (strings.last_known) }
                        }
                        h1.long[answer.chars().count() > LONG_ANSWER_AFTER] {
                            span.question { (strings.question) }
                            " "
                            (answer)
                        }
                        @for line in closure_lines(strings, headline) {
                            p.closure { (line) }
                        }
                        @if let Some(schedule) = headline.schedule {
                            p.now { (strings.schedule_line(schedule)) }
                        }
                        @if let Freshness::Checked(checked) = &freshness {
                            p.checked { (checked.line(strings)) }
                            @if checked.stale() {
                                p.stale {
                                    (strings.stale)
                                    " "
                                    a href=(SOURCE_URL) { (strings.stale_link) }
                                }
                            }
                        }
                        section.alerts {
                            h2 { (strings.alerts) }
                            p.alert.(tone.class) {
                                span.icon aria-hidden="true" { (tone.icon) }
                                (strings.alert_line(headline.alerts))
                            }
                            @if no_closure_hours {
                                p.no-closure-hours { (strings.no_closure_hours) }
                            }
                            (details)
                        }
                        (next_hours)
                        section.schedule {
                            h2 { (strings.opening_hours) }
                            table.opening-hours {
                                @for season in &schedule::SEASONS {
                                    tr.row {
                                        th scope="row" {
                                            (strings.month(season.first_month))
                                            "–"
                                            (strings.month(season.last_month))
                                        }
                                        td { ((strings.season_hours)(season)) }
                                    }
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
                        " "
                        (strings.forecast_source_lead)
                        a href=(FORECAST_SOURCE_URL) { "Open-Meteo" }
                        " ("
                        a href=(strings.licence_url) { "CC BY 4.0" }
                        ")."
                    }
                }
            }
        }
    }
}
