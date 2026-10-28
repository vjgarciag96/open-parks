use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use jiff::Timestamp;
use open_parks::page;
use open_parks::poller::{Fetched, Latest};
use open_parks::public_url::PublicUrl;
use open_parks::sigma;
use reqwest::Url;
use tokio::sync::watch;
use tower::ServiceExt;

use crate::support::fixtures::fixture;

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
    let reopens = Some("Cerrado por horario\u{a0}· Abre a las 06:00");
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
            Some("Cerrado por horario\u{a0}· Abre a las 06:00"),
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
async fn the_page_has_no_scripts_and_inline_styles_only() {
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert!(!body.contains("<script"), "{body}");
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
    let reopens = Some("Closed for the night\u{a0}· Opens at 06:00");
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
            schedule: Some("Closed for the night\u{a0}· Opens at 06:00"),
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
        body.contains("<dt>Fechas</dt><dd>12/10/2026</dd>"),
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
    let body = text_body(get(fetched("code_1_open.json"), "/es").await).await;

    assert!(
        body.contains("<dt>Fechas</dt><dd>10/10/2026</dd>"),
        "{body}"
    );
    for term in ["Horario de la incidencia", "Reapertura", "Observaciones"] {
        assert!(
            !body.contains(&format!("<dt>{term}</dt>")),
            "{term}: {body}"
        );
    }
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
                r#"Actualizado el <time datetime="2026-10-11T09:30:00Z">11/10/2026 a las 11:30 (hora de Madrid)</time>{separator}<span class="ago">{ago}</span>"#,
                separator = "\u{a0}· ",
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
        "<dt>Dates</dt><dd>13/10/2026</dd>",
        "<dt>Reopening</dt><dd>See the notes</dd>",
        "<dt>Notes</dt><dd>El parque permanecerá cerrado hasta la retirada de las ramas caídas.</dd>",
        concat!(
            r#"Updated <time datetime="2026-10-11T09:30:00Z">11/10/2026 at 11:30 Madrid time</time>"#,
            "\u{a0}· ",
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
async fn the_parks_hours_follow_the_alerts() {
    let body = text_body(get(fetched("code_6_red_alert_restrictions.json"), "/es").await).await;

    assert!(
        body.contains(r#"</span></p></section><section class="schedule">"#),
        "{body}"
    );
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
            "<title>¿Está abierto el Retiro hoy? Parque del Retiro, Madrid</title>",
            r#"<meta name="description" content="Consulta si el Parque del Retiro de Madrid está abierto, su horario y si hay cierres o restricciones por alerta meteorológica, con datos del Ayuntamiento actualizados cada minuto.">"#,
        ),
        (
            "/en",
            "<title>Is Retiro Park open today? Live status, Madrid</title>",
            r#"<meta name="description" content="Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data refreshed every minute.">"#,
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
async fn robots_txt_keeps_crawlers_off_the_api_and_points_to_the_sitemap() {
    let response = get_with_public_url(fetched("code_1_open.json"), "/robots.txt").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(
        text_body(response).await,
        "User-agent: *\nDisallow: /api/\nSitemap: https://retiro.example/sitemap.xml\n"
    );
}

#[tokio::test]
async fn robots_txt_has_no_sitemap_without_a_public_url() {
    let body = text_body(get(fetched("code_1_open.json"), "/robots.txt").await).await;

    assert_eq!(body, "User-agent: *\nDisallow: /api/\n");
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
