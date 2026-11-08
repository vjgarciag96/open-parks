use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use jiff::Timestamp;
use jiff::civil::date;
use open_parks::park::forecast::{Forecast, HourForecast};
use open_parks::sigma::{Fetched, Latest};
use open_parks::web::page;
use open_parks::web::public_url::PublicUrl;
use open_parks::{open_meteo, sigma};
use reqwest::Url;
use tokio::sync::watch;
use tower::ServiceExt;

use crate::support::fixtures::{fixture, open_meteo_fixture};

const FETCHED_AT: &str = "2026-10-11T09:30:00Z";
const PUBLIC_URL: &str = "https://retiro.example";

async fn send(
    latest: Latest,
    now: Timestamp,
    telegram_bot_username: Option<&str>,
    request: Request<Body>,
) -> Response {
    send_to(latest, now, telegram_bot_username, None, request).await
}

async fn send_to(
    latest: Latest,
    now: Timestamp,
    telegram_bot_username: Option<&str>,
    public_url: Option<&str>,
    request: Request<Body>,
) -> Response {
    page::router(
        watch::channel(latest).1,
        watch::channel(None).1,
        move || now,
        telegram_bot_username.map(str::to_owned),
        public_url.map(public),
    )
    .oneshot(request)
    .await
    .unwrap()
}

fn public(url: &str) -> PublicUrl {
    PublicUrl::new(Url::parse(url).unwrap()).unwrap()
}

async fn get_with_public_url(latest: Latest, uri: &str) -> Response {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    send_to(
        latest,
        FETCHED_AT.parse().unwrap(),
        None,
        Some(PUBLIC_URL),
        request,
    )
    .await
}

async fn get(latest: Latest, uri: &str) -> Response {
    get_at(latest, FETCHED_AT.parse().unwrap(), uri).await
}

async fn get_at(latest: Latest, now: Timestamp, uri: &str) -> Response {
    send(
        latest,
        now,
        None,
        Request::get(uri).body(Body::empty()).unwrap(),
    )
    .await
}

async fn get_with_bot(latest: Latest, uri: &str) -> Response {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    send(
        latest,
        FETCHED_AT.parse().unwrap(),
        Some("retiro_abierto_bot"),
        request,
    )
    .await
}

async fn get_accepting(accept_language: &str) -> Response {
    let request = Request::get("/")
        .header(header::ACCEPT_LANGUAGE, accept_language)
        .body(Body::empty())
        .unwrap();
    send(fetched("code_1_open.json"), Timestamp::now(), None, request).await
}

fn fetched(fixture_name: &str) -> Latest {
    Ok(Fetched {
        status: sigma::decode(&fixture(fixture_name)).unwrap(),
        fetched_at: FETCHED_AT.parse().unwrap(),
    })
}

fn h1(question: &str, answer: &str) -> String {
    format!(r#"<span class="question">{question}</span> {answer}</h1>"#)
}

fn assert_html(response: &Response) {
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
}

async fn body_at(fixture_name: &str, now: &str, uri: &str) -> String {
    text_body(get_at(fetched(fixture_name), now.parse().unwrap(), uri).await).await
}

async fn text_body(response: Response) -> String {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn the_status_page_is_served_as_html() {
    let response = get(fetched("code_7_closed.json"), "/es").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_html(&response);
    let body = text_body(response).await;
    assert!(body.starts_with("<!DOCTYPE html>"), "{body}");
    assert!(body.contains("Cerrado"), "{body}");
}

const WITHIN_HOURS: &str = FETCHED_AT;
const OUTSIDE_HOURS: &str = "2026-10-11T21:30:00Z";
const WITHIN_SUMMER_HOURS: &str = "2026-07-11T09:30:00Z";

struct Headline {
    answer: &'static str,
    alert: &'static str,
    schedule: Option<&'static str>,
}

fn alerts_box(heading: &str, alert: &str) -> [String; 2] {
    [
        format!(r#"<section class="alerts"><h2>{heading}</h2><p class="alert "#),
        format!(r#"</span>{alert}</p>"#),
    ]
}

fn schedule_line(schedule: Option<&str>) -> String {
    match schedule {
        Some(schedule) => format!(r#"</h1><p class="now">{schedule}</p><section class="alerts">"#),
        None => r#"</h1><section class="alerts">"#.to_owned(),
    }
}

fn schedule_box(heading: &str) -> String {
    format!(r#"<section class="schedule"><h2>{heading}</h2><table class="opening-hours">"#)
}

fn assert_headline(body: &str, question: &str, (alerts, hours): (&str, &str), expected: &Headline) {
    let [alerts_heading, alert] = alerts_box(alerts, expected.alert);
    for part in [
        h1(question, expected.answer),
        schedule_line(expected.schedule),
        alerts_heading,
        alert,
        schedule_box(hours),
    ] {
        assert!(body.contains(&part), "{part}: {body}");
    }
}

fn assert_spanish_headline(body: &str, expected: &Headline) {
    assert_headline(
        body,
        "¿Está abierto el Retiro?",
        ("Alertas", "Horario"),
        expected,
    );
}

fn assert_english_headline(body: &str, expected: &Headline) {
    assert_headline(body, "Is Retiro Park open?", ("Alerts", "Hours"), expected);
}

#[tokio::test]
async fn within_hours_the_answer_is_whether_the_alerts_let_the_park_open() {
    let cases = [
        (
            "code_1_open.json",
            "Abierto",
            "Sin alertas",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_2_incidents.json",
            "Abierto",
            "Con restricciones",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_3_yellow_alert.json",
            "Abierto",
            "Con restricciones",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_4_orange_alert.json",
            "Abierto",
            "No se recomienda permanecer",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_5_red_alert_closure.json",
            "Abierto, cierre previsto por alerta",
            "Cierre previsto",
            None,
        ),
        (
            "code_6_red_alert_restrictions.json",
            "Abierto",
            "Con restricciones",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_7_closed.json",
            "Cerrado",
            "Cerrado por alerta meteorológica",
            None,
        ),
        (
            "unknown_codes.json",
            "Estado desconocido",
            "Estado desconocido",
            Some("Cierra a las 22:00"),
        ),
    ];
    for (name, answer, alert, schedule) in cases {
        let body = body_at(name, WITHIN_HOURS, "/es").await;

        assert_spanish_headline(
            &body,
            &Headline {
                answer,
                alert,
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn outside_hours_the_park_is_closed_whatever_the_alerts() {
    let reopens = Some("Cerrado por horario. Abre a las 06:00");
    let cases = [
        ("code_1_open.json", "Sin alertas", reopens),
        ("code_2_incidents.json", "Con restricciones", reopens),
        ("code_3_yellow_alert.json", "Con restricciones", reopens),
        (
            "code_4_orange_alert.json",
            "No se recomienda permanecer",
            reopens,
        ),
        (
            "code_5_red_alert_closure.json",
            "Cierre previsto",
            Some("Cerrado por horario"),
        ),
        (
            "code_6_red_alert_restrictions.json",
            "Con restricciones",
            reopens,
        ),
        (
            "code_7_closed.json",
            "Cerrado por alerta meteorológica",
            None,
        ),
        ("unknown_codes.json", "Estado desconocido", reopens),
    ];
    for (name, alert, schedule) in cases {
        let body = body_at(name, OUTSIDE_HOURS, "/es").await;

        assert_spanish_headline(
            &body,
            &Headline {
                answer: "Cerrado",
                alert,
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn in_summer_the_park_closes_at_midnight() {
    let spanish = body_at("code_1_open.json", WITHIN_SUMMER_HOURS, "/es").await;
    let english = body_at("code_1_open.json", WITHIN_SUMMER_HOURS, "/en").await;

    assert!(
        spanish.contains(&schedule_line(Some("Cierra a medianoche"))),
        "{spanish}"
    );
    assert!(
        english.contains(&schedule_line(Some("Closes at midnight"))),
        "{english}"
    );
}

#[tokio::test]
async fn opening_time_never_shows_during_a_weather_closure_or_forecast() {
    for name in ["code_5_red_alert_closure.json", "code_7_closed.json"] {
        let spanish = body_at(name, OUTSIDE_HOURS, "/es").await;
        let english = body_at(name, OUTSIDE_HOURS, "/en").await;

        assert!(!spanish.contains("Abre a las"), "fixture {name}: {spanish}");
        assert!(!english.contains("Opens at"), "fixture {name}: {english}");
    }
}

#[tokio::test]
async fn a_failing_upstream_says_the_alerts_could_not_be_checked() {
    let cases = [
        (
            WITHIN_HOURS,
            "No hemos podido consultar el estado",
            Some("Cierra a las 22:00"),
        ),
        (
            OUTSIDE_HOURS,
            "Cerrado",
            Some("Cerrado por horario. Abre a las 06:00"),
        ),
    ];
    for (now, answer, schedule) in cases {
        let response = get_at(
            Err(sigma::FetchError::BodyTooLarge),
            now.parse().unwrap(),
            "/es",
        )
        .await;

        let body = text_body(response).await;
        assert_spanish_headline(
            &body,
            &Headline {
                answer,
                alert: "No hemos podido consultar si hay alertas",
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn a_failing_upstream_is_a_bad_gateway_page() {
    let response = get(Err(sigma::FetchError::BodyTooLarge), "/es").await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_html(&response);
    let body = text_body(response).await;
    assert!(
        body.contains(&h1(
            "¿Está abierto el Retiro?",
            "No hemos podido consultar el estado"
        )),
        "{body}"
    );
}

#[tokio::test]
async fn an_upstream_that_never_responds_is_a_gateway_timeout_page() {
    let response = get(Err(sigma::FetchError::Timeout), "/es").await;

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = text_body(response).await;
    assert!(
        body.contains(&h1(
            "¿Está abierto el Retiro?",
            "No hemos podido consultar el estado"
        )),
        "{body}"
    );
}

#[tokio::test]
async fn the_page_runs_no_scripts_and_has_inline_styles_only() {
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert_eq!(
        body.matches("<script").count(),
        body.matches(r#"<script type="application/ld+json">"#)
            .count(),
        "{body}"
    );
    assert!(!body.contains(r#"rel="stylesheet""#), "{body}");
    assert!(body.contains("<style>"), "{body}");
}

#[tokio::test]
async fn the_page_credits_madrid_as_the_source_under_its_licence() {
    for latest in [fetched("code_1_open.json"), Err(sigma::FetchError::Timeout)] {
        let body = text_body(get(latest, "/es").await).await;

        assert!(
            body.contains(
                r#"Elaborado a partir de <a href="https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&amp;vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&amp;vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&amp;vgnextlocale=es">datos del Ayuntamiento de Madrid</a>"#
            ),
            "{body}"
        );
        assert!(
            body.contains(
                r#"<a href="https://creativecommons.org/licenses/by/4.0/deed.es">CC BY 4.0</a>"#
            ),
            "{body}"
        );
        assert!(body.contains("no está vinculada al Ayuntamiento"), "{body}");
    }
}

#[tokio::test]
async fn within_hours_the_english_answer_is_whether_the_alerts_let_the_park_open() {
    let cases = [
        (
            "code_1_open.json",
            "Open",
            "No alerts",
            Some("Closes at 22:00"),
        ),
        (
            "code_2_incidents.json",
            "Open",
            "Restrictions in place",
            Some("Closes at 22:00"),
        ),
        (
            "code_3_yellow_alert.json",
            "Open",
            "Restrictions in place",
            Some("Closes at 22:00"),
        ),
        (
            "code_4_orange_alert.json",
            "Open",
            "Staying is not advised",
            Some("Closes at 22:00"),
        ),
        (
            "code_5_red_alert_closure.json",
            "Open, closure expected due to an alert",
            "Closure expected",
            None,
        ),
        (
            "code_6_red_alert_restrictions.json",
            "Open",
            "Restrictions in place",
            Some("Closes at 22:00"),
        ),
        (
            "code_7_closed.json",
            "Closed",
            "Closed due to a weather alert",
            None,
        ),
        (
            "unknown_codes.json",
            "Status unknown",
            "Status unknown",
            Some("Closes at 22:00"),
        ),
    ];
    for (name, answer, alert, schedule) in cases {
        let response = get_at(fetched(name), WITHIN_HOURS.parse().unwrap(), "/en").await;

        assert_eq!(response.status(), StatusCode::OK, "fixture {name}");
        let body = text_body(response).await;
        assert!(
            body.contains(r#"<html lang="en">"#),
            "fixture {name}: {body}"
        );
        assert_english_headline(
            &body,
            &Headline {
                answer,
                alert,
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn outside_hours_the_english_answer_is_closed_whatever_the_alerts() {
    let reopens = Some("Closed for the night. Opens at 06:00");
    let cases = [
        ("code_1_open.json", "No alerts", reopens),
        ("code_2_incidents.json", "Restrictions in place", reopens),
        ("code_3_yellow_alert.json", "Restrictions in place", reopens),
        (
            "code_4_orange_alert.json",
            "Staying is not advised",
            reopens,
        ),
        (
            "code_5_red_alert_closure.json",
            "Closure expected",
            Some("Closed for the night"),
        ),
        (
            "code_6_red_alert_restrictions.json",
            "Restrictions in place",
            reopens,
        ),
        ("code_7_closed.json", "Closed due to a weather alert", None),
        ("unknown_codes.json", "Status unknown", reopens),
    ];
    for (name, alert, schedule) in cases {
        let body = body_at(name, OUTSIDE_HOURS, "/en").await;

        assert_english_headline(
            &body,
            &Headline {
                answer: "Closed",
                alert,
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn a_failing_upstream_outside_hours_is_explained_in_english() {
    let response = get_at(
        Err(sigma::FetchError::Timeout),
        OUTSIDE_HOURS.parse().unwrap(),
        "/en",
    )
    .await;

    let body = text_body(response).await;
    assert_english_headline(
        &body,
        &Headline {
            answer: "Closed",
            alert: "We couldn’t check for weather alerts",
            schedule: Some("Closed for the night. Opens at 06:00"),
        },
    );
}

#[tokio::test]
async fn the_root_redirects_to_spanish_without_a_language_preference() {
    let response = get(fetched("code_1_open.json"), "/").await;

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers()[header::LOCATION], "/es");
    assert_eq!(response.headers()[header::VARY], "accept-language");
}

#[tokio::test]
async fn the_root_redirects_to_the_preferred_language() {
    let cases = [
        ("en", "/en"),
        ("en-GB,en;q=0.9", "/en"),
        ("EN-us", "/en"),
        ("fr-FR, en;q=0.8, es;q=0.5", "/en"),
        ("es-ES,es;q=0.9,en;q=0.8", "/es"),
        ("en;q=0.5, es", "/es"),
        ("en;q=0", "/es"),
        ("fr, de", "/es"),
        ("*", "/es"),
        ("", "/es"),
        ("en;q=garbage", "/es"),
        ("en;Q=0", "/es"),
        ("en; q = 0", "/es"),
        ("en;q=2, es", "/es"),
        ("en;q=inf, es", "/es"),
        ("en;q=-1", "/es"),
    ];
    for (accept_language, location) in cases {
        let response = get_accepting(accept_language).await;

        assert_eq!(response.status(), StatusCode::FOUND, "{accept_language:?}");
        assert_eq!(
            response.headers()[header::LOCATION],
            location,
            "{accept_language:?}"
        );
        assert_eq!(
            response.headers()[header::VARY],
            "accept-language",
            "{accept_language:?}"
        );
    }
}

#[tokio::test]
async fn each_page_links_to_the_other_language() {
    for timeout in [false, true] {
        let latest = || match timeout {
            false => fetched("code_1_open.json"),
            true => Err(sigma::FetchError::Timeout),
        };
        let spanish = text_body(get(latest(), "/es").await).await;
        let english = text_body(get(latest(), "/en").await).await;

        assert!(
            spanish.contains(r#"<a href="/en" hreflang="en" lang="en">English</a>"#),
            "{spanish}"
        );
        assert!(
            english.contains(r#"<a href="/es" hreflang="es" lang="es">Español</a>"#),
            "{english}"
        );
    }
}

#[tokio::test]
async fn a_failing_upstream_is_explained_in_english() {
    let response = get(Err(sigma::FetchError::Timeout), "/en").await;

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = text_body(response).await;
    assert!(
        body.contains(&h1("Is Retiro Park open?", "We couldn’t check the status")),
        "{body}"
    );
}

#[tokio::test]
async fn the_english_page_credits_madrid_as_the_source_under_its_licence() {
    for latest in [fetched("code_1_open.json"), Err(sigma::FetchError::Timeout)] {
        let body = text_body(get(latest, "/en").await).await;

        assert!(
            body.contains(r#"Based on <a href="https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&amp;vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&amp;vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&amp;vgnextlocale=es">data from Madrid City Council (Ayuntamiento de Madrid)</a>"#),
            "{body}"
        );
        assert!(
            body.contains(
                r#"<a href="https://creativecommons.org/licenses/by/4.0/deed.en">CC BY 4.0</a>"#
            ),
            "{body}"
        );
        assert!(
            body.contains("not affiliated with or endorsed by the City Council"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn the_page_shows_the_status_details() {
    let body = text_body(get(fetched("code_6_red_alert_restrictions.json"), "/es").await).await;

    assert!(
        body.contains("<dt>Fechas de la incidencia</dt><dd>12/10/2026</dd>"),
        "{body}"
    );
    assert!(
        body.contains("<dt>Horario de la incidencia</dt><dd>de 14:00 a 20:00 h</dd>"),
        "{body}"
    );
    assert!(
        body.contains("<dt>Observaciones</dt><dd>Cerradas las zonas infantiles, deportivas y de mayores. Suspendidos los eventos al aire libre.</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn every_reopening_forecast_is_shown() {
    let cases = [
        (
            "code_5_red_alert_closure.json",
            "Cuando terminen las inspecciones",
        ),
        ("code_7_closed.json", "Consulta las observaciones"),
        ("unknown_codes.json", "Desconocida"),
    ];
    for (name, reopening) in cases {
        let body = text_body(get(fetched(name), "/es").await).await;

        assert!(
            body.contains(&format!("<dt>Reapertura</dt><dd>{reopening}</dd>")),
            "fixture {name}: {body}"
        );
    }
}

#[tokio::test]
async fn absent_details_are_left_out() {
    let body = text_body(get(fetched("code_5_red_alert_closure.json"), "/es").await).await;

    assert!(
        body.contains("<dt>Fechas de la incidencia</dt><dd>12/10/2026</dd>"),
        "{body}"
    );
    for term in ["Horario de la incidencia", "Observaciones"] {
        assert!(
            !body.contains(&format!("<dt>{term}</dt>")),
            "{term}: {body}"
        );
    }
}

#[tokio::test]
async fn an_open_park_shows_no_incident_dates() {
    // code_1_open.json carries a date, as SIGMA sends with no incident.
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert!(!body.contains("10/10/2026"), "{body}");
    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn an_open_park_shows_no_incident_hours() {
    // code_1_open_with_hours.json carries hours, as SIGMA can send with no
    // incident.
    let body = text_body(get(fetched("code_1_open_with_hours.json"), "/es").await).await;

    assert!(!body.contains("03:00 a 12:00"), "{body}");
    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn a_status_without_details_shows_no_details_list() {
    let body = text_body(get(fetched("blank_strings.json"), "/es").await).await;

    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn the_page_says_when_and_how_long_ago_the_status_was_fetched() {
    let cases = [
        ("2026-10-11T09:30:00Z", "hace menos de un minuto"),
        ("2026-10-11T09:30:59Z", "hace menos de un minuto"),
        ("2026-10-11T09:31:00Z", "hace 1 minuto"),
        ("2026-10-11T09:35:20Z", "hace 5 minutos"),
        ("2026-10-11T10:29:59Z", "hace 59 minutos"),
        ("2026-10-11T10:30:00Z", "hace 1 hora"),
        ("2026-10-11T13:10:00Z", "hace 3 horas"),
        ("2026-10-12T09:30:00Z", "hace 1 día"),
        ("2026-10-13T11:00:00Z", "hace 2 días"),
        // A clock that has stepped back since the fetch.
        ("2026-10-11T09:29:00Z", "hace menos de un minuto"),
    ];
    for (now, ago) in cases {
        let body = body_at("code_1_open.json", now, "/es").await;

        assert!(
            body.contains(&format!(
                r#"Actualizado el <time datetime="2026-10-11T09:30:00Z">11/10/2026 a las 11:30 (hora de Madrid)</time>, <span class="ago">{ago}</span>"#,
            )),
            "now {now}: {body}"
        );
    }
}

#[tokio::test]
async fn the_fetch_time_is_in_madrid_time_whatever_the_season() {
    let cases = [
        // Summer time, two hours ahead of UTC.
        ("2026-07-11T09:30:00Z", "11/07/2026 a las 11:30"),
        // Winter time, one hour ahead.
        ("2026-12-11T09:30:00Z", "11/12/2026 a las 10:30"),
        // Already the next day, and year, in Madrid.
        ("2026-12-31T23:30:00Z", "01/01/2027 a las 00:30"),
    ];
    for (fetched_at, expected) in cases {
        let latest = Ok(Fetched {
            status: sigma::decode(&fixture("code_1_open.json")).unwrap(),
            fetched_at: fetched_at.parse().unwrap(),
        });
        let body = text_body(get_at(latest, fetched_at.parse().unwrap(), "/es").await).await;

        assert!(
            body.contains(&format!(
                r#"<time datetime="{fetched_at}">{expected} (hora de Madrid)</time>"#
            )),
            "{fetched_at}: {body}"
        );
    }
}

#[tokio::test]
async fn a_status_fetched_ten_minutes_ago_or_more_is_flagged_as_stale() {
    let stale = "Puede que este estado no esté al día.";
    let cases = [
        ("2026-10-11T09:39:59Z", false),
        ("2026-10-11T09:40:00Z", true),
        ("2026-10-12T09:30:00Z", true),
    ];
    for (now, flagged) in cases {
        let body = body_at("code_1_open.json", now, "/es").await;

        assert_eq!(body.contains(stale), flagged, "now {now}: {body}");
    }
}

#[tokio::test]
async fn the_status_details_are_shown_in_english() {
    let body = body_at("code_7_closed.json", "2026-10-11T12:00:00Z", "/en").await;

    for expected in [
        "<dt>Incident dates</dt><dd>13/10/2026</dd>",
        "<dt>Reopening</dt><dd>See the notes</dd>",
        "<dt>Notes</dt><dd>El parque permanecerá cerrado hasta la retirada de las ramas caídas.</dd>",
        concat!(
            r#"Updated <time datetime="2026-10-11T09:30:00Z">11/10/2026 at 11:30 Madrid time</time>"#,
            ", ",
            r#"<span class="ago">2 hours ago</span>"#,
        ),
        "This status may be out of date.",
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn the_incident_hours_are_labelled_apart_from_the_parks_hours_in_english() {
    let body = text_body(get(fetched("code_6_red_alert_restrictions.json"), "/en").await).await;

    assert!(
        body.contains("<dt>Incident hours</dt><dd>de 14:00 a 20:00 h</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn each_page_shows_the_parks_hours_in_both_seasons() {
    let cases = [
        (
            "/es",
            r#"<section class="schedule"><h2>Horario</h2><table class="opening-hours"><tr class="row"><th scope="row">Octubre–Marzo</th><td>6:00–22:00</td></tr><tr class="row"><th scope="row">Abril–Septiembre</th><td>6:00–24:00</td></tr></table>"#,
        ),
        (
            "/en",
            r#"<section class="schedule"><h2>Hours</h2><table class="opening-hours"><tr class="row"><th scope="row">October–March</th><td>6:00–22:00</td></tr><tr class="row"><th scope="row">April–September</th><td>6:00–midnight</td></tr></table>"#,
        ),
    ];
    for (uri, hours) in cases {
        for latest in [
            || fetched("code_6_red_alert_restrictions.json"),
            || Err(sigma::FetchError::Timeout),
        ] {
            let body = text_body(get(latest(), uri).await).await;

            assert!(body.contains(hours), "{uri}: {body}");
        }
    }
}

#[tokio::test]
async fn the_next_hours_card_sits_between_the_alerts_and_the_parks_hours() {
    let cases = [
        (
            "/es",
            concat!(
                r#"</span></p></section><section class="next-hours"><h2>Próximas horas</h2>"#,
                r#"<p class="unavailable">La previsión no está disponible.</p></section>"#,
                r#"<section class="schedule">"#,
            ),
        ),
        (
            "/en",
            concat!(
                r#"</span></p></section><section class="next-hours"><h2>Next hours</h2>"#,
                r#"<p class="unavailable">The forecast isn’t available.</p></section>"#,
                r#"<section class="schedule">"#,
            ),
        ),
    ];
    for (uri, expected) in cases {
        let body = text_body(get(fetched("code_6_red_alert_restrictions.json"), uri).await).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_page_links_to_the_telegram_bot_when_there_is_one() {
    for latest in [
        || fetched("code_1_open.json"),
        || Err(sigma::FetchError::Timeout),
    ] {
        let spanish = text_body(get_with_bot(latest(), "/es").await).await;
        let english = text_body(get_with_bot(latest(), "/en").await).await;

        assert!(
            spanish
                .contains(r#"<a href="https://t.me/retiro_abierto_bot"><svg aria-hidden="true""#),
            "{spanish}"
        );
        assert!(
            english
                .contains(r#"<a href="https://t.me/retiro_abierto_bot"><svg aria-hidden="true""#),
            "{english}"
        );
        assert!(
            spanish.contains("</svg>Recibe avisos en Telegram</a>"),
            "{spanish}"
        );
        assert!(
            english.contains("</svg>Get notified on Telegram</a>"),
            "{english}"
        );
    }
}

#[tokio::test]
async fn the_page_has_no_telegram_link_without_a_bot() {
    for latest in [
        || fetched("code_1_open.json"),
        || Err(sigma::FetchError::Timeout),
    ] {
        for uri in ["/es", "/en"] {
            let body = text_body(get(latest(), uri).await).await;

            assert!(!body.contains("t.me"), "{uri}: {body}");
            assert!(!body.contains("Telegram"), "{uri}: {body}");
        }
    }
}

#[tokio::test]
async fn each_page_has_a_title_and_description_in_its_language() {
    let cases = [
        (
            "/es",
            "<title>¿Está abierto hoy el Parque del Retiro de Madrid?</title>",
            r#"<meta name="description" content="Consulta si el Parque del Retiro de Madrid está abierto, su horario y los cierres o restricciones por alerta meteorológica, con datos del Ayuntamiento.">"#,
        ),
        (
            "/en",
            "<title>Is Madrid’s Retiro Park open today?</title>",
            r#"<meta name="description" content="Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data.">"#,
        ),
    ];
    for (uri, title, description) in cases {
        for latest in [
            || fetched("code_1_open.json"),
            || Err(sigma::FetchError::Timeout),
        ] {
            let body = text_body(get(latest(), uri).await).await;

            assert!(body.contains(title), "{uri}: {body}");
            assert!(body.contains(description), "{uri}: {body}");
        }
    }
}

#[tokio::test]
async fn each_page_keeps_its_text_out_of_search_snippets() {
    for uri in ["/es", "/en"] {
        let body = text_body(get(fetched("code_1_open.json"), uri).await).await;

        assert!(
            body.contains("<body><div data-nosnippet><nav>"),
            "{uri}: {body}"
        );
    }
}

#[tokio::test]
async fn each_page_links_its_canonical_url_and_alternates_with_a_public_url() {
    for (uri, canonical) in [
        ("/es", "https://retiro.example/es"),
        ("/en", "https://retiro.example/en"),
    ] {
        for latest in [
            || fetched("code_1_open.json"),
            || Err(sigma::FetchError::Timeout),
        ] {
            let body = text_body(get_with_public_url(latest(), uri).await).await;

            for expected in [
                format!(r#"<link rel="canonical" href="{canonical}">"#),
                r#"<link rel="alternate" hreflang="es" href="https://retiro.example/es"><link rel="alternate" hreflang="en" href="https://retiro.example/en"><link rel="alternate" hreflang="x-default" href="https://retiro.example/">"#.to_owned(),
                format!(r#"<meta property="og:url" content="{canonical}">"#),
            ] {
                assert!(body.contains(&expected), "{uri}: {expected}: {body}");
            }
        }
    }
}

#[tokio::test]
async fn the_page_has_no_absolute_links_to_itself_without_a_public_url() {
    for uri in ["/es", "/en"] {
        let body = text_body(get(fetched("code_1_open.json"), uri).await).await;

        assert!(!body.contains("canonical"), "{uri}: {body}");
        assert!(!body.contains(r#"rel="alternate""#), "{uri}: {body}");
        assert!(!body.contains("og:url"), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_link_preview_names_the_cause_of_a_closure() {
    let cases = [
        (
            "code_1_open.json",
            WITHIN_HOURS,
            "¿Está abierto el Retiro? Abierto",
        ),
        (
            "code_1_open.json",
            OUTSIDE_HOURS,
            "¿Está abierto el Retiro? Cerrado por horario",
        ),
        (
            "code_5_red_alert_closure.json",
            OUTSIDE_HOURS,
            "¿Está abierto el Retiro? Cerrado por horario",
        ),
        (
            "code_7_closed.json",
            WITHIN_HOURS,
            "¿Está abierto el Retiro? Cerrado por alerta meteorológica",
        ),
        (
            "code_7_closed.json",
            OUTSIDE_HOURS,
            "¿Está abierto el Retiro? Cerrado por alerta meteorológica",
        ),
    ];
    for (name, now, title) in cases {
        let body = body_at(name, now, "/es").await;

        let expected = format!(r#"<meta property="og:title" content="{title}">"#);
        assert!(body.contains(&expected), "fixture {name} at {now}: {body}");
    }
}

#[tokio::test]
async fn the_link_preview_is_in_the_pages_language() {
    let spanish = body_at("code_1_open.json", OUTSIDE_HOURS, "/es").await;
    let english = body_at("code_1_open.json", OUTSIDE_HOURS, "/en").await;

    for expected in [
        r#"<meta property="og:type" content="website">"#,
        r#"<meta property="og:title" content="¿Está abierto el Retiro? Cerrado por horario">"#,
        r#"<meta property="og:description" content="Consulta si el Parque del Retiro de Madrid está abierto, su horario y"#,
        r#"<meta property="og:locale" content="es_ES"><meta property="og:locale:alternate" content="en_GB">"#,
    ] {
        assert!(spanish.contains(expected), "{expected}: {spanish}");
    }
    for expected in [
        r#"<meta property="og:title" content="Is Retiro Park open? Closed for the night">"#,
        r#"<meta property="og:description" content="Check whether Madrid’s Retiro Park is open, its opening hours and"#,
        r#"<meta property="og:locale" content="en_GB"><meta property="og:locale:alternate" content="es_ES">"#,
    ] {
        assert!(english.contains(expected), "{expected}: {english}");
    }
}

#[tokio::test]
async fn the_page_links_its_favicon() {
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert!(
        body.contains(r#"<link rel="icon" href="/favicon.svg" type="image/svg+xml">"#),
        "{body}"
    );
}

#[tokio::test]
async fn the_favicon_is_served_as_svg() {
    let response = get(fetched("code_1_open.json"), "/favicon.svg").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/svg+xml");
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=86400"
    );
    let body = text_body(response).await;
    assert!(
        body.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "{body}"
    );
}

#[tokio::test]
async fn robots_txt_lets_crawlers_read_everything_and_points_to_the_sitemap() {
    let response = get_with_public_url(fetched("code_1_open.json"), "/robots.txt").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(
        text_body(response).await,
        "User-agent: *\nAllow: /\nSitemap: https://retiro.example/sitemap.xml\n"
    );
}

#[tokio::test]
async fn robots_txt_has_no_sitemap_without_a_public_url() {
    let body = text_body(get(fetched("code_1_open.json"), "/robots.txt").await).await;

    assert_eq!(body, "User-agent: *\nAllow: /\n");
}

#[tokio::test]
async fn the_sitemap_lists_each_language_with_its_alternates() {
    let response = get_with_public_url(fetched("code_1_open.json"), "/sitemap.xml").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/xml");
    let alternates = concat!(
        r#"<xhtml:link rel="alternate" hreflang="es" href="https://retiro.example/es"/>"#,
        r#"<xhtml:link rel="alternate" hreflang="en" href="https://retiro.example/en"/>"#,
        r#"<xhtml:link rel="alternate" hreflang="x-default" href="https://retiro.example/"/>"#,
    );
    assert_eq!(
        text_body(response).await,
        format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">"#,
                "<url><loc>https://retiro.example/es</loc>{alternates}</url>",
                "<url><loc>https://retiro.example/en</loc>{alternates}</url>",
                "</urlset>",
            ),
            alternates = alternates
        )
    );
}

#[tokio::test]
async fn there_is_no_sitemap_without_a_public_url() {
    let response = get(fetched("code_1_open.json"), "/sitemap.xml").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

fn structured_data(body: &str) -> serde_json::Value {
    let (_, rest) = body
        .split_once(r#"<script type="application/ld+json">"#)
        .unwrap_or_else(|| panic!("no structured data: {body}"));
    let (json, _) = rest.split_once("</script>").unwrap();
    serde_json::from_str(json).unwrap()
}

#[tokio::test]
async fn each_page_describes_the_park_as_structured_data_in_its_language() {
    for (uri, name) in [("/es", "Parque del Retiro"), ("/en", "Retiro Park")] {
        for latest in [
            || fetched("code_1_open.json"),
            || Err(sigma::FetchError::Timeout),
        ] {
            let park = structured_data(&text_body(get(latest(), uri).await).await);

            assert_eq!(park["@context"], "https://schema.org", "{uri}: {park}");
            assert_eq!(park["@type"], "Park", "{uri}: {park}");
            assert_eq!(park["name"], name, "{uri}: {park}");
            assert_eq!(
                park["alternateName"], "Jardines de El Buen Retiro",
                "{uri}: {park}"
            );
            assert_eq!(
                park["sameAs"], "https://www.wikidata.org/wiki/Q1131807",
                "{uri}: {park}"
            );
            assert_eq!(
                park["address"],
                serde_json::json!({
                    "@type": "PostalAddress",
                    "streetAddress": "Plaza de la Independencia, 7",
                    "postalCode": "28001",
                    "addressLocality": "Madrid",
                    "addressCountry": "ES",
                }),
                "{uri}: {park}"
            );
            assert_eq!(
                park["geo"],
                serde_json::json!({
                    "@type": "GeoCoordinates",
                    "latitude": 40.41964087412762,
                    "longitude": -3.68793370943788,
                }),
                "{uri}: {park}"
            );
        }
    }
}

fn season(opens: &str, closes: &str, from: &str, through: &str) -> serde_json::Value {
    serde_json::json!({
        "@type": "OpeningHoursSpecification",
        "dayOfWeek": [
            "https://schema.org/Monday",
            "https://schema.org/Tuesday",
            "https://schema.org/Wednesday",
            "https://schema.org/Thursday",
            "https://schema.org/Friday",
            "https://schema.org/Saturday",
            "https://schema.org/Sunday",
        ],
        "opens": opens,
        "closes": closes,
        "validFrom": from,
        "validThrough": through,
    })
}

#[tokio::test]
async fn the_structured_data_gives_the_hours_of_this_season_and_the_next() {
    let winter = || season("06:00", "22:00", "2026-10-01", "2027-03-31");
    let summer = || season("06:00", "00:00", "2027-04-01", "2027-09-30");
    let cases = [
        // Already October in Madrid.
        ("2026-09-30T22:00:00Z", [winter(), summer()]),
        ("2027-03-31T21:59:59Z", [winter(), summer()]),
        (
            "2027-04-01T00:00:00+02:00",
            [
                summer(),
                season("06:00", "22:00", "2027-10-01", "2028-03-31"),
            ],
        ),
        (
            "2026-09-30T21:59:59Z",
            [
                season("06:00", "00:00", "2026-04-01", "2026-09-30"),
                winter(),
            ],
        ),
    ];
    for (now, expected) in cases {
        let park = structured_data(&body_at("code_1_open.json", now, "/es").await);

        assert_eq!(
            park["openingHoursSpecification"],
            serde_json::json!(expected),
            "at {now}"
        );
    }
}

#[tokio::test]
async fn the_structured_data_leaves_out_closures_for_alerts() {
    let body = body_at("code_7_closed.json", FETCHED_AT, "/es").await;
    let park = structured_data(&body);

    assert!(
        park.get("specialOpeningHoursSpecification").is_none(),
        "{park}"
    );
}

#[tokio::test]
async fn the_structured_data_links_the_page_with_a_public_url() {
    for (uri, url) in [
        ("/es", "https://retiro.example/es"),
        ("/en", "https://retiro.example/en"),
    ] {
        let park = structured_data(
            &text_body(get_with_public_url(fetched("code_1_open.json"), uri).await).await,
        );

        assert_eq!(park["url"], url, "{uri}: {park}");
    }
}

#[tokio::test]
async fn the_structured_data_has_no_url_without_a_public_url() {
    let park = structured_data(&text_body(get(fetched("code_1_open.json"), "/es").await).await);

    assert!(park.get("url").is_none(), "{park}");
}

#[tokio::test]
async fn each_page_names_the_site_with_a_public_url() {
    for uri in ["/es", "/en"] {
        let body = text_body(get_with_public_url(fetched("code_1_open.json"), uri).await).await;

        assert!(
            body.contains(r#"<meta property="og:site_name" content="Retiro Abierto">"#),
            "{uri}: {body}"
        );
        assert!(
            body.contains(r#"{"@context":"https://schema.org","@type":"WebSite","name":"Retiro Abierto","url":"https://retiro.example/"}"#),
            "{uri}: {body}"
        );
    }
}

#[tokio::test]
async fn the_site_is_not_described_without_a_public_url() {
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert!(!body.contains("WebSite"), "{body}");
}

#[tokio::test]
async fn llms_txt_says_what_the_site_is_and_links_its_pages_and_api() {
    let request = Request::get("/llms.txt").body(Body::empty()).unwrap();
    let response = send_to(
        fetched("code_1_open.json"),
        FETCHED_AT.parse().unwrap(),
        Some("retiro_abierto_bot"),
        Some(PUBLIC_URL),
        request,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = text_body(response).await;
    assert!(body.starts_with("# Retiro Abierto\n\n> "), "{body}");
    for expected in [
        "- October–March: 6:00–22:00\n- April–September: 6:00–midnight\n",
        "[Status page in English](https://retiro.example/en)",
        "[Status page in Spanish](https://retiro.example/es)",
        "[Status API](https://retiro.example/api/v1/parks/retiro/status)",
        "[Telegram bot](https://t.me/retiro_abierto_bot)",
        "[Madrid City Council's park closures page](https://www.madrid.es/",
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn llms_txt_links_relatively_without_a_public_url_and_skips_a_missing_bot() {
    let body = text_body(get(fetched("code_1_open.json"), "/llms.txt").await).await;

    assert!(body.contains("[Status page in English](/en)"), "{body}");
    assert!(
        body.contains("[Status API](/api/v1/parks/retiro/status)"),
        "{body}"
    );
    assert!(!body.contains("Telegram"), "{body}");
}

const FORECAST_FETCHED_AT: &str = "2026-11-02T09:15:00+01:00";

fn forecast(fixture_name: &str) -> open_meteo::Latest {
    Some(open_meteo::Fetched {
        forecast: open_meteo::decode(&open_meteo_fixture(fixture_name)).unwrap(),
        fetched_at: FORECAST_FETCHED_AT.parse().unwrap(),
    })
}

fn forecast_of(hour: HourForecast) -> open_meteo::Latest {
    Some(open_meteo::Fetched {
        forecast: Forecast { hours: vec![hour] },
        fetched_at: FORECAST_FETCHED_AT.parse().unwrap(),
    })
}

fn hour_at_nine() -> HourForecast {
    HourForecast {
        at: date(2026, 11, 2)
            .at(9, 0, 0, 0)
            .to_zoned(jiff::tz::get!("Europe/Madrid"))
            .unwrap(),
        temperature: Some(7.3),
        weather_code: Some(0),
        is_day: Some(true),
        rain_chance: Some(30),
        gusts: Some(16.6),
    }
}

fn hour_at(year: i16, month: i8, day: i8, hour: i8) -> HourForecast {
    HourForecast {
        at: date(year, month, day)
            .at(hour, 0, 0, 0)
            .to_zoned(jiff::tz::get!("Europe/Madrid"))
            .unwrap(),
        ..hour_at_nine()
    }
}

async fn get_with_forecast(
    latest: Latest,
    forecast: open_meteo::Latest,
    now: &str,
    uri: &str,
) -> Response {
    let now: Timestamp = now.parse().unwrap();
    page::router(
        watch::channel(latest).1,
        watch::channel(forecast).1,
        move || now,
        None,
        None,
    )
    .oneshot(Request::get(uri).body(Body::empty()).unwrap())
    .await
    .unwrap()
}

async fn body_with_forecast(forecast: open_meteo::Latest, now: &str, uri: &str) -> String {
    text_body(get_with_forecast(fetched("code_1_open.json"), forecast, now, uri).await).await
}

#[tokio::test]
async fn the_card_shows_twelve_columns_with_the_forecasts_values_starting_with_now() {
    let body = body_with_forecast(
        forecast("rain_chances.json"),
        "2026-11-02T09:30:00+01:00",
        "/en",
    )
    .await
    .replace(" risky", "");

    assert_eq!(body.matches(r#"<span class="hour">"#).count(), 12, "{body}");
    let first = concat!(
        r#"<ol class="hours" tabindex="0" aria-label="Hourly forecast">"#,
        r#"<li class="current"><span class="day">Today</span><span class="hour">Now</span><span class="icon"><svg "#,
    );
    let first_values = concat!(
        r#"</svg></span><span class="temperature">7°</span>"#,
        r#"<span class="rain">70%</span><span class="gusts">17</span></li>"#,
    );
    let second_values = concat!(
        r#"</svg></span><span class="temperature">11°</span>"#,
        r#"<span class="rain">77%</span><span class="gusts">24</span></li>"#,
    );
    let last_values = concat!(
        r#"</svg></span><span class="temperature">14°</span>"#,
        r#"<span class="rain">46%</span><span class="gusts">6</span></li></ol>"#,
    );
    for expected in [first, first_values, second_values, last_values] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn the_card_has_a_legend_in_each_language() {
    let cases = [
        ("/es", "Probabilidad de lluvia. Rachas en km/h"),
        ("/en", "Rain chance. Gusts in km/h"),
    ];
    for (uri, legend) in cases {
        let body =
            body_with_forecast(forecast("retiro.json"), "2026-11-02T09:30:00+01:00", uri).await;

        assert!(
            body.contains(&format!(r#"</ol><p class="legend">{legend}</p>"#)),
            "{uri}: {body}"
        );
    }
}

#[tokio::test]
async fn the_current_hour_shows_today_above_now_in_each_language() {
    let cases = [
        (
            "/es",
            r#"<li class="current"><span class="day">Hoy</span><span class="hour">Ahora</span>"#,
        ),
        (
            "/en",
            r#"<li class="current"><span class="day">Today</span><span class="hour">Now</span>"#,
        ),
    ];
    for (uri, expected) in cases {
        let body =
            body_with_forecast(forecast("retiro.json"), "2026-11-02T09:30:00+01:00", uri).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn before_opening_the_opening_hour_shows_today_in_madrid_time() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T05:30:00+01:00", "/en").await;

    assert!(
        body.contains(
            r#"aria-label="Hourly forecast"><li class=""><span class="day">Today</span><span class="hour">6:00</span>"#
        ),
        "{body}"
    );
}

#[tokio::test]
async fn the_night_shows_as_one_closed_block_before_the_next_days_first_hour_in_each_language() {
    let cases = [
        (
            "/es",
            r#"<li class="closed">Cerrado 22–6</li><li class=""><span class="day">mar</span><span class="hour">6:00</span>"#,
        ),
        (
            "/en",
            r#"<li class="closed">Closed 22–6</li><li class=""><span class="day">Tue</span><span class="hour">6:00</span>"#,
        ),
    ];
    for (uri, expected) in cases {
        let body =
            body_with_forecast(forecast("retiro.json"), "2026-11-02T20:30:00+01:00", uri).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_closed_block_does_not_count_towards_the_twelve_hours() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T20:30:00+01:00", "/en").await;

    assert_eq!(body.matches(r#"<span class="hour">"#).count(), 12, "{body}");
}

#[tokio::test]
async fn only_the_first_column_and_the_first_hour_of_the_next_day_show_a_day() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T20:30:00+01:00", "/en").await;

    assert_eq!(body.matches(r#"<span class="day">"#).count(), 2, "{body}");
}

#[tokio::test]
async fn the_summer_night_shows_as_a_block_from_midnight() {
    let forecast = Some(open_meteo::Fetched {
        forecast: Forecast {
            hours: vec![hour_at(2026, 7, 3, 23), hour_at(2026, 7, 4, 6)],
        },
        fetched_at: "2026-07-03T22:00:00+02:00".parse().unwrap(),
    });

    let body = body_with_forecast(forecast, "2026-07-03T23:10:00+02:00", "/en").await;

    assert!(
        body.contains(
            r#"<li class="closed">Closed 0–6</li><li class=""><span class="day">Sat</span>"#
        ),
        "{body}"
    );
}

#[tokio::test]
async fn while_closed_the_card_starts_at_opening_time_with_no_now() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T23:00:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"aria-label="Hourly forecast"><li class="">"#,
            r#"<span class="day">Tue</span><span class="hour">6:00</span>"#,
        )),
        "{body}"
    );
    assert!(!body.contains(r#"<span class="hour">Now</span>"#), "{body}");
}

#[tokio::test]
async fn each_column_shows_its_weathers_icon() {
    let hour = HourForecast {
        weather_code: Some(0),
        is_day: Some(true),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(&format!(
            r#"<span class="hour">Now</span><span class="icon">{}</span>"#,
            include_str!("../../assets/weather/clear-sky.svg")
        )),
        "{body}"
    );
}

#[tokio::test]
async fn a_night_hour_shows_the_night_icon() {
    let hour = HourForecast {
        weather_code: Some(0),
        is_day: Some(false),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(&format!(
            r#"<span class="icon">{}</span>"#,
            include_str!("../../assets/weather/clear-sky-night.svg")
        )),
        "{body}"
    );
}

#[tokio::test]
async fn a_missing_rain_chance_or_gust_shows_a_dash() {
    let hour = HourForecast {
        rain_chance: None,
        gusts: None,
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"<span class="temperature">7°</span>"#,
            r#"<span class="rain">–</span>"#,
            r#"<span class="gusts">–</span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn an_unknown_weather_code_shows_a_dash_for_its_icon_and_keeps_the_values() {
    let hour = HourForecast {
        weather_code: Some(4),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"<span class="hour">Now</span><span class="icon">–</span>"#,
            r#"<span class="temperature">7°</span>"#,
            r#"<span class="rain">30%</span>"#,
            r#"<span class="gusts">17</span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn a_missing_temperature_blanks_the_column_but_keeps_its_hour() {
    let hour = HourForecast {
        temperature: None,
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"<li class="current"><span class="day">Today</span><span class="hour">Now</span><span class="icon"></span>"#,
            r#"<span class="temperature"></span><span class="rain"></span>"#,
            r#"<span class="gusts"></span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn a_missing_weather_code_blanks_the_column_but_keeps_its_hour() {
    let hour = HourForecast {
        weather_code: None,
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"<li class="current"><span class="day">Today</span><span class="hour">Now</span><span class="icon"></span>"#,
            r#"<span class="temperature"></span><span class="rain"></span>"#,
            r#"<span class="gusts"></span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn a_rain_chance_of_zero_is_shown_rather_than_left_blank() {
    let hour = HourForecast {
        rain_chance: Some(0),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(body.contains(r#"<span class="rain">0%</span>"#), "{body}");
}

#[tokio::test]
async fn a_temperature_just_below_zero_rounds_to_zero_without_a_sign() {
    let hour = HourForecast {
        temperature: Some(-0.4),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(body.contains(r#">0°</span>"#), "{body}");
}

#[tokio::test]
async fn gusts_at_the_cut_off_are_highlighted() {
    let hour = HourForecast {
        gusts: Some(40.0),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<span class="gusts risky">40</span>"#),
        "{body}"
    );
}

#[tokio::test]
async fn gusts_below_the_cut_off_are_not_highlighted() {
    let hour = HourForecast {
        gusts: Some(39.4),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(body.contains(r#"<span class="gusts">39</span>"#), "{body}");
}

#[tokio::test]
async fn a_rain_chance_at_the_cut_off_is_highlighted() {
    let hour = HourForecast {
        rain_chance: Some(60),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<span class="rain risky">60%</span>"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_rain_chance_below_the_cut_off_is_not_highlighted() {
    let hour = HourForecast {
        rain_chance: Some(59),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(body.contains(r#"<span class="rain">59%</span>"#), "{body}");
}

#[tokio::test]
async fn a_temperature_at_the_hot_cut_off_is_highlighted() {
    let hour = HourForecast {
        temperature: Some(35.0),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<span class="temperature risky">35°</span>"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_temperature_between_the_cut_offs_is_not_highlighted() {
    let hour = HourForecast {
        temperature: Some(34.4),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<span class="temperature">34°</span>"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_temperature_below_zero_keeps_its_sign() {
    let hour = HourForecast {
        temperature: Some(-0.6),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(body.contains(r#">-1°</span>"#), "{body}");
}

#[tokio::test]
async fn the_card_says_the_forecast_is_unavailable_before_the_first_one_arrives() {
    let body = body_with_forecast(None, "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<p class="unavailable">The forecast isn’t available.</p>"#),
        "{body}"
    );
    assert!(!body.contains("<ol"), "{body}");
    assert!(!body.contains(r#"class="legend""#), "{body}");
}

#[tokio::test]
async fn the_card_says_the_forecast_is_unavailable_when_no_hour_contains_now() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-04T00:00:00+01:00", "/es").await;

    assert!(
        body.contains(r#"<p class="unavailable">La previsión no está disponible.</p>"#),
        "{body}"
    );
    assert!(!body.contains("<ol"), "{body}");
    assert!(!body.contains("Previsión de hace"), "{body}");
}

#[tokio::test]
async fn a_forecast_three_hours_old_shows_no_age() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T12:15:00+01:00", "/en").await;

    assert!(body.contains("<ol"), "{body}");
    assert!(!body.contains(r#"class="forecast-age""#), "{body}");
}

#[tokio::test]
async fn a_forecast_over_three_hours_old_shows_its_columns_and_its_age() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T14:15:00+01:00", "/en").await;

    assert!(body.contains(r#"<span class="hour">Now</span>"#), "{body}");
    assert!(
        body.contains(r#"<p class="forecast-age">Forecast from 5 hours ago</p>"#),
        "{body}"
    );
}

#[tokio::test]
async fn the_age_of_an_old_forecast_is_shown_in_spanish() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T14:15:00+01:00", "/es").await;

    assert!(
        body.contains(r#"<p class="forecast-age">Previsión de hace 5 horas</p>"#),
        "{body}"
    );
}

#[tokio::test]
async fn the_age_note_sits_under_the_legend() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T14:15:00+01:00", "/en").await;

    assert!(
        body.contains(concat!(
            r#"<p class="legend">Rain chance. Gusts in km/h</p>"#,
            r#"<p class="forecast-age">"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn an_old_forecast_has_fewer_columns_once_its_data_runs_out() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-03T18:15:00+01:00", "/en").await;

    assert_eq!(body.matches(r#"<span class="hour">"#).count(), 4, "{body}");
}

#[tokio::test]
async fn the_status_is_the_same_with_and_without_a_forecast() {
    let now = "2026-11-02T09:30:00+01:00";
    let open = get_with_forecast(fetched("code_1_open.json"), None, now, "/es").await;
    let open_with_forecast = get_with_forecast(
        fetched("code_1_open.json"),
        forecast("retiro.json"),
        now,
        "/es",
    )
    .await;

    assert_eq!(open.status(), StatusCode::OK);
    assert_eq!(open_with_forecast.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_failing_status_keeps_its_gateway_status_with_and_without_a_forecast() {
    let now = "2026-11-02T09:30:00+01:00";
    let without = get_with_forecast(Err(sigma::FetchError::Timeout), None, now, "/es").await;
    let with = get_with_forecast(
        Err(sigma::FetchError::Timeout),
        forecast("retiro.json"),
        now,
        "/es",
    )
    .await;

    assert_eq!(without.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(with.status(), StatusCode::GATEWAY_TIMEOUT);
}

#[tokio::test]
async fn the_card_still_shows_the_forecast_when_the_status_fails() {
    let response = get_with_forecast(
        Err(sigma::FetchError::Timeout),
        forecast("retiro.json"),
        "2026-11-02T09:30:00+01:00",
        "/en",
    )
    .await;

    let body = text_body(response).await;
    assert!(body.contains(r#"<span class="hour">Now</span>"#), "{body}");
}

#[tokio::test]
async fn the_footer_credits_open_meteo_after_the_disclaimer_under_its_licence() {
    let cases = [
        (
            "/es",
            concat!(
                "avalada por él. Previsión de ",
                r#"<a href="https://open-meteo.com/">Open-Meteo</a> ("#,
                r#"<a href="https://creativecommons.org/licenses/by/4.0/deed.es">CC BY 4.0</a>)."#,
                "</footer>",
            ),
        ),
        (
            "/en",
            concat!(
                "endorsed by the City Council. Forecast from ",
                r#"<a href="https://open-meteo.com/">Open-Meteo</a> ("#,
                r#"<a href="https://creativecommons.org/licenses/by/4.0/deed.en">CC BY 4.0</a>)."#,
                "</footer>",
            ),
        ),
    ];
    for (uri, credit) in cases {
        let body = body_with_forecast(None, "2026-11-02T09:30:00+01:00", uri).await;

        assert!(body.contains(credit), "{uri}: {body}");
    }
}
