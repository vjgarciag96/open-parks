use std::collections::BTreeSet;
use std::fs;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use jiff::Timestamp;
use jiff::civil::date;
use open_parks::park::forecast::{Forecast, HourForecast};
use open_parks::park::status::{AlertCode, ParkStatus, ReopeningForecast};
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
    get_with_public_url_at(latest, FETCHED_AT.parse().unwrap(), uri).await
}

async fn get_with_public_url_at(latest: Latest, now: Timestamp, uri: &str) -> Response {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    send_to(latest, now, None, Some(PUBLIC_URL), request).await
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
    let response = get(fetched("code_7_closed.json"), "/").await;

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
    /// The lines under the answer on an alert's closure, in order.
    closure: &'static [&'static str],
    alert: &'static str,
    schedule: Option<&'static str>,
    checked: Checked,
}

/// How recent the answer says it is, for a status fetched at `FETCHED_AT`.
#[derive(Clone, Copy)]
enum Checked {
    Fresh,
    Stale,
    Never,
}

struct PageCopy {
    question: &'static str,
    alerts: &'static str,
    hours: &'static str,
    last_known: &'static str,
    checked_at_fetched_at: &'static str,
    stale: &'static str,
}

const SPANISH: PageCopy = PageCopy {
    question: "¿Está abierto el Retiro?",
    alerts: "Alertas",
    hours: "Horario",
    last_known: "Último estado conocido",
    checked_at_fetched_at: "Última comprobación a las 11:30",
    stale: "Puede que este estado no esté al día. <a href=\"https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&amp;vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&amp;vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&amp;vgnextlocale=es\">Consulta la web del Ayuntamiento</a>",
};

const ENGLISH: PageCopy = PageCopy {
    question: "Is Retiro Park open?",
    alerts: "Alerts",
    hours: "Hours",
    last_known: "Last known status",
    checked_at_fetched_at: "Last checked at 11:30",
    stale: "This status may be out of date. <a href=\"https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&amp;vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&amp;vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&amp;vgnextlocale=es\">Check the City Council’s page</a>",
};

fn alerts_box(heading: &str, alert: &str) -> [String; 2] {
    [
        format!(r#"<section class="alerts"><h2>{heading}</h2><p class="alert "#),
        format!(r#"</span>{alert}</p>"#),
    ]
}

/// The schedule line right under the answer.
fn schedule_line(schedule: Option<&str>) -> String {
    match schedule {
        Some(schedule) => format!(r#"</h1><p class="now">{schedule}</p>"#),
        None => "</h1>".to_owned(),
    }
}

/// Everything from the answer to the alerts card, in order.
fn under_the_answer(copy: &PageCopy, expected: &Headline) -> String {
    let closure: String = expected
        .closure
        .iter()
        .map(|line| format!(r#"<p class="closure">{line}</p>"#))
        .collect();
    let schedule = expected
        .schedule
        .map(|schedule| format!(r#"<p class="now">{schedule}</p>"#))
        .unwrap_or_default();
    let checked = format!(r#"<p class="checked">{}</p>"#, copy.checked_at_fetched_at);
    let checked = match expected.checked {
        Checked::Fresh => checked,
        Checked::Stale => format!(r#"{checked}<p class="stale">{}</p>"#, copy.stale),
        Checked::Never => String::new(),
    };
    format!(r#"</h1>{closure}{schedule}{checked}<section class="alerts">"#)
}

fn schedule_box(heading: &str) -> String {
    format!(r#"<section class="schedule"><h2>{heading}</h2><table class="opening-hours">"#)
}

fn assert_headline(body: &str, copy: &PageCopy, expected: &Headline) {
    let [alerts_heading, alert] = alerts_box(copy.alerts, expected.alert);
    for part in [
        h1(copy.question, expected.answer),
        under_the_answer(copy, expected),
        alerts_heading,
        alert,
        schedule_box(copy.hours),
    ] {
        assert!(body.contains(&part), "{part}: {body}");
    }
    let last_known = format!(r#"<p class="last-known">{}</p><h1"#, copy.last_known);
    assert_eq!(
        body.contains(&last_known),
        matches!(expected.checked, Checked::Stale),
        "{last_known}: {body}"
    );
}

fn assert_spanish_headline(body: &str, expected: &Headline) {
    assert_headline(body, &SPANISH, expected);
}

fn assert_english_headline(body: &str, expected: &Headline) {
    assert_headline(body, &ENGLISH, expected);
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
            "Abierto con restricciones",
            "Con restricciones",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_3_yellow_alert.json",
            "Abierto con restricciones",
            "Con restricciones",
            Some("Cierra a las 22:00"),
        ),
        (
            "code_4_orange_alert.json",
            "Abierto, pero no se recomienda permanecer",
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
            "Abierto, cierre previsto por alerta",
            "Cierre previsto",
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
        let body = body_at(name, WITHIN_HOURS, "/").await;

        assert_spanish_headline(
            &body,
            &Headline {
                checked: Checked::Fresh,
                closure: &[],
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
            "Cierre previsto",
            Some("Cerrado por horario"),
        ),
        ("unknown_codes.json", "Estado desconocido", reopens),
    ];
    for (name, alert, schedule) in cases {
        let body = body_at(name, OUTSIDE_HOURS, "/").await;

        assert_spanish_headline(
            &body,
            &Headline {
                checked: Checked::Stale,
                closure: &[],
                answer: "Cerrado",
                alert,
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn in_summer_the_park_closes_at_midnight() {
    let spanish = body_at("code_1_open.json", WITHIN_SUMMER_HOURS, "/").await;
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
        let spanish = body_at(name, OUTSIDE_HOURS, "/").await;
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
            "/",
        )
        .await;

        let body = text_body(response).await;
        assert_spanish_headline(
            &body,
            &Headline {
                checked: Checked::Never,
                closure: &[],
                answer,
                alert: "No hemos podido consultar si hay alertas",
                schedule,
            },
        );
    }
}

#[tokio::test]
async fn a_failing_upstream_is_a_bad_gateway_page() {
    let response = get(Err(sigma::FetchError::BodyTooLarge), "/").await;

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
    let response = get(Err(sigma::FetchError::Timeout), "/").await;

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
    let body = text_body(get(fetched("code_1_open.json"), "/").await).await;

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
        let body = text_body(get(latest, "/").await).await;

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

// The live capture's window is 12:00-24:00 on 11/11/2026, Madrid time.
const BEFORE_THE_ALERT: &str = "2026-11-11T10:30:00Z";
const DURING_THE_ALERT: &str = "2026-11-11T19:06:00Z";

#[tokio::test]
async fn a_forecast_closure_answers_closed_once_its_window_is_under_way() {
    let cases = [
        (
            "/",
            BEFORE_THE_ALERT,
            Checked::Fresh,
            "Abierto, cierre previsto por alerta",
            &["Cierre previsto de 12:00 a medianoche"][..],
            "Cierre previsto",
        ),
        (
            "/",
            DURING_THE_ALERT,
            Checked::Stale,
            "Cerrado",
            &["Por alerta meteorológica", "Durante el resto del día"],
            "Cerrado por alerta meteorológica",
        ),
        (
            "/en",
            BEFORE_THE_ALERT,
            Checked::Fresh,
            "Open, closure expected due to an alert",
            &["Closure expected from 12:00 to midnight"],
            "Closure expected",
        ),
        (
            "/en",
            DURING_THE_ALERT,
            Checked::Stale,
            "Closed",
            &["Due to a weather alert", "For the rest of the day"],
            "Closed due to a weather alert",
        ),
    ];
    for (uri, now, checked, answer, closure, alert) in cases {
        // Fetched the morning of the alert, at the same time of day as
        // FETCHED_AT.
        let latest = Ok(Fetched {
            status: sigma::decode(&fixture("code_6_live_closure.json")).unwrap(),
            fetched_at: BEFORE_THE_ALERT.parse().unwrap(),
        });
        let body = text_body(get_at(latest, now.parse().unwrap(), uri).await).await;
        let expected = Headline {
            checked,
            closure,
            answer,
            alert,
            schedule: None,
        };

        if uri == "/" {
            assert_spanish_headline(&body, &expected);
        } else {
            assert_english_headline(&body, &expected);
        }
    }
}

#[tokio::test]
async fn an_alert_closure_says_its_cause_and_when_it_reopens_under_the_answer() {
    let cases = [
        (
            WITHIN_HOURS,
            "/",
            Headline {
                checked: Checked::Fresh,
                answer: "Cerrado",
                closure: &[
                    "Por alerta meteorológica",
                    "Reapertura: consulta las observaciones",
                ],
                alert: "Cerrado por alerta meteorológica",
                schedule: None,
            },
        ),
        (
            OUTSIDE_HOURS,
            "/",
            Headline {
                checked: Checked::Stale,
                answer: "Cerrado",
                closure: &[
                    "Por alerta meteorológica",
                    "Reapertura: consulta las observaciones",
                ],
                alert: "Cerrado por alerta meteorológica",
                schedule: None,
            },
        ),
        (
            WITHIN_HOURS,
            "/en",
            Headline {
                checked: Checked::Fresh,
                answer: "Closed",
                closure: &["Due to a weather alert", "Reopening: see the notes"],
                alert: "Closed due to a weather alert",
                schedule: None,
            },
        ),
        (
            OUTSIDE_HOURS,
            "/en",
            Headline {
                checked: Checked::Stale,
                answer: "Closed",
                closure: &["Due to a weather alert", "Reopening: see the notes"],
                alert: "Closed due to a weather alert",
                schedule: None,
            },
        ),
    ];
    for (now, uri, expected) in cases {
        let body = body_at("code_7_closed.json", now, uri).await;

        if uri == "/" {
            assert_spanish_headline(&body, &expected);
        } else {
            assert_english_headline(&body, &expected);
        }
    }
}

fn closed_by_alert(reopening: Option<ReopeningForecast>) -> Latest {
    Ok(Fetched {
        status: ParkStatus {
            code: AlertCode::Closed,
            dates: Some("13/10/2026".to_owned()),
            hours: None,
            reopening,
            notes: None,
        },
        fetched_at: FETCHED_AT.parse().unwrap(),
    })
}

#[tokio::test]
async fn an_alert_closure_after_inspections_says_it_reopens_once_they_finish() {
    let cases = [
        (
            "/",
            r#"</h1><p class="closure">Por alerta meteorológica</p><p class="closure">Reabrirá cuando terminen las inspecciones</p><p class="checked">"#,
        ),
        (
            "/en",
            r#"</h1><p class="closure">Due to a weather alert</p><p class="closure">Reopens once inspections finish</p><p class="checked">"#,
        ),
    ];
    for (uri, expected) in cases {
        let latest = closed_by_alert(Some(ReopeningForecast::AfterInspections));

        let body = text_body(get(latest, uri).await).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn an_alert_closure_with_no_known_reopening_gives_only_its_cause() {
    for reopening in [None, Some(ReopeningForecast::Unknown(9))] {
        let latest = closed_by_alert(reopening);

        let body = text_body(get(latest, "/").await).await;

        assert!(
            body.contains(
                r#"</h1><p class="closure">Por alerta meteorológica</p><p class="checked">"#
            ),
            "{reopening:?}: {body}"
        );
    }
}

fn alert(
    code: AlertCode,
    dates: &str,
    hours: &str,
    reopening: Option<ReopeningForecast>,
) -> Latest {
    Ok(Fetched {
        status: ParkStatus {
            code,
            dates: Some(dates.to_owned()),
            hours: Some(hours.to_owned()),
            reopening,
            notes: None,
        },
        fetched_at: FETCHED_AT.parse().unwrap(),
    })
}

/// The lines right under the answer on an alert's closure, in order.
fn closure_lines(body: &str) -> Vec<&str> {
    let (_, mut rest) = body.split_once("</h1>").unwrap();
    let mut lines = Vec::new();
    while let Some(line) = rest.strip_prefix(r#"<p class="closure">"#) {
        let (line, after) = line.split_once("</p>").unwrap();
        lines.push(line);
        rest = after;
    }
    lines
}

#[tokio::test]
async fn an_expected_closure_on_a_later_day_gives_no_closure_line() {
    // 11:30 and 18:00 in Madrid the day before the live capture's window.
    for now in ["2026-11-10T10:30:00Z", "2026-11-10T17:00:00Z"] {
        let body = body_at("code_6_live_closure.json", now, "/").await;

        assert_eq!(closure_lines(&body), Vec::<&str>::new(), "at {now}");
    }
}

#[tokio::test]
async fn an_alert_closure_ending_before_closing_time_says_until_when() {
    let cases = [
        ("/", ["Por alerta meteorológica", "Hasta las 18:00"]),
        ("/en", ["Due to a weather alert", "Until 18:00"]),
    ];
    for (uri, expected) in cases {
        let latest = alert(
            AlertCode::RedAlertRestrictions,
            "11/11/2026",
            "12:00-18:00",
            None,
        );

        let body =
            text_body(get_at(latest, "2026-11-11T14:00:00Z".parse().unwrap(), uri).await).await;

        assert_eq!(closure_lines(&body), expected, "{uri}");
    }
}

#[tokio::test]
async fn an_alert_closure_past_closing_time_says_it_lasts_the_rest_of_the_day() {
    // 15:00 and 22:30 in Madrid, the park closing at 22:00.
    let cases = [
        (
            "/",
            "2026-11-11T14:00:00Z",
            ["Por alerta meteorológica", "Durante el resto del día"],
        ),
        (
            "/",
            "2026-11-11T21:30:00Z",
            ["Por alerta meteorológica", "Durante el resto del día"],
        ),
        (
            "/en",
            "2026-11-11T14:00:00Z",
            ["Due to a weather alert", "For the rest of the day"],
        ),
    ];
    for (uri, now, expected) in cases {
        let body = body_at("code_6_live_closure.json", now, uri).await;

        assert_eq!(closure_lines(&body), expected, "{uri} at {now}");
    }
}

#[tokio::test]
async fn an_alert_closure_ending_as_the_park_closes_at_midnight_says_until_midnight() {
    let cases = [
        ("/", ["Por alerta meteorológica", "Hasta medianoche"]),
        ("/en", ["Due to a weather alert", "Until midnight"]),
    ];
    for (uri, expected) in cases {
        let latest = alert(
            AlertCode::RedAlertRestrictions,
            "11/07/2026",
            "12:00-24:00",
            None,
        );

        let body =
            text_body(get_at(latest, "2026-07-11T13:00:00Z".parse().unwrap(), uri).await).await;

        assert_eq!(closure_lines(&body), expected, "{uri}");
    }
}

#[tokio::test]
async fn an_alert_closure_with_hours_says_when_it_ends_rather_than_its_reopening_forecast() {
    let latest = alert(
        AlertCode::RedAlertClosure,
        "11/11/2026",
        "12:00-18:00",
        Some(ReopeningForecast::AfterInspections),
    );

    let body = text_body(get_at(latest, "2026-11-11T14:00:00Z".parse().unwrap(), "/").await).await;

    assert_eq!(
        closure_lines(&body),
        ["Por alerta meteorológica", "Hasta las 18:00"]
    );
}

#[tokio::test]
async fn an_alert_closure_whose_window_has_not_begun_says_when_it_reopens() {
    let latest = alert(
        AlertCode::Closed,
        "11/11/2026",
        "18:00-24:00",
        Some(ReopeningForecast::AfterInspections),
    );

    let body = text_body(get_at(latest, BEFORE_THE_ALERT.parse().unwrap(), "/").await).await;

    assert_eq!(
        closure_lines(&body),
        [
            "Por alerta meteorológica",
            "Reabrirá cuando terminen las inspecciones"
        ]
    );
}

#[tokio::test]
async fn an_ended_window_gives_no_closure_line_nor_says_the_hours_are_missing() {
    let latest = alert(
        AlertCode::RedAlertRestrictions,
        "11/11/2026",
        "03:00 a 12:00",
        None,
    );

    let body = text_body(get_at(latest, "2026-11-11T14:00:00Z".parse().unwrap(), "/").await).await;

    assert_eq!(closure_lines(&body), Vec::<&str>::new());
    assert!(!body.contains("no ha indicado el horario"), "{body}");
}

#[tokio::test]
async fn at_night_an_expected_closure_gives_no_closure_line() {
    // 05:00 in Madrid on the live capture's day, before the park opens.
    let body = body_at("code_6_live_closure.json", "2026-11-11T04:00:00Z", "/").await;

    assert!(
        body.contains(&schedule_line(Some("Cerrado por horario"))),
        "{body}"
    );
}

#[tokio::test]
async fn an_expected_closure_without_hours_says_the_city_council_has_not_given_them() {
    let cases = [
        (
            "/",
            r#"</span>Cierre previsto</p><p class="no-closure-hours">El Ayuntamiento no ha indicado el horario del cierre.</p>"#,
        ),
        (
            "/en",
            r#"</span>Closure expected</p><p class="no-closure-hours">The City Council hasn’t given the closure hours.</p>"#,
        ),
    ];
    for (uri, expected) in cases {
        let body = body_at(
            "code_5_live_forecast_without_hours.json",
            "2026-11-18T10:30:00Z",
            uri,
        )
        .await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn an_expected_closure_with_unreadable_hours_does_not_say_they_are_missing() {
    let body = body_at("code_6_red_alert_restrictions.json", WITHIN_HOURS, "/").await;

    assert!(!body.contains("no ha indicado el horario"), "{body}");
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
            "Open with restrictions",
            "Restrictions in place",
            Some("Closes at 22:00"),
        ),
        (
            "code_3_yellow_alert.json",
            "Open with restrictions",
            "Restrictions in place",
            Some("Closes at 22:00"),
        ),
        (
            "code_4_orange_alert.json",
            "Open, but staying is not advised",
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
            "Open, closure expected due to an alert",
            "Closure expected",
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
                checked: Checked::Fresh,
                closure: &[],
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
            "Closure expected",
            Some("Closed for the night"),
        ),
        ("unknown_codes.json", "Status unknown", reopens),
    ];
    for (name, alert, schedule) in cases {
        let body = body_at(name, OUTSIDE_HOURS, "/en").await;

        assert_english_headline(
            &body,
            &Headline {
                checked: Checked::Stale,
                closure: &[],
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
            checked: Checked::Never,
            closure: &[],
            answer: "Closed",
            alert: "We couldn’t check for weather alerts",
            schedule: Some("Closed for the night. Opens at 06:00"),
        },
    );
}

#[tokio::test]
async fn the_root_serves_spanish_whatever_the_language_preference() {
    for accept_language in ["en", "en-GB,en;q=0.9", "es", "fr", ""] {
        let response = get_accepting(accept_language).await;

        assert_eq!(response.status(), StatusCode::OK, "{accept_language:?}");
        let body = text_body(response).await;
        assert!(
            body.contains(r#"<html lang="es">"#),
            "{accept_language:?}: {body}"
        );
    }
}

#[tokio::test]
async fn the_old_spanish_address_redirects_permanently_to_the_root() {
    let response = get(fetched("code_1_open.json"), "/es").await;

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn the_english_address_with_a_trailing_slash_redirects_permanently_without_it() {
    let response = get(fetched("code_1_open.json"), "/en/").await;

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers()[header::LOCATION], "/en");
}

#[tokio::test]
async fn the_old_spanish_address_with_a_trailing_slash_redirects_permanently_to_the_root() {
    let response = get(fetched("code_1_open.json"), "/es/").await;

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn each_redirect_keeps_the_query_string() {
    let cases = [
        ("/en/?x=1", "/en?x=1"),
        ("/es/?x=1", "/?x=1"),
        ("/es?x=1", "/?x=1"),
    ];
    for (uri, location) in cases {
        let response = get(fetched("code_1_open.json"), uri).await;

        assert_eq!(
            response.headers()[header::LOCATION],
            location,
            "redirect from {uri}"
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
        let spanish = text_body(get(latest(), "/").await).await;
        let english = text_body(get(latest(), "/en").await).await;

        assert!(
            spanish.contains(r#"<a href="/en" hreflang="en" lang="en">English</a>"#),
            "{spanish}"
        );
        assert!(
            english.contains(r#"<a href="/" hreflang="es" lang="es">Español</a>"#),
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
    let body = text_body(get(fetched("code_6_red_alert_restrictions.json"), "/").await).await;

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
        let body = text_body(get(fetched(name), "/").await).await;

        assert!(
            body.contains(&format!("<dt>Reapertura</dt><dd>{reopening}</dd>")),
            "fixture {name}: {body}"
        );
    }
}

#[tokio::test]
async fn absent_details_are_left_out() {
    let body = text_body(get(fetched("code_5_red_alert_closure.json"), "/").await).await;

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
    let body = text_body(get(fetched("code_1_open.json"), "/").await).await;

    assert!(!body.contains("10/10/2026"), "{body}");
    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn an_open_park_shows_no_incident_hours() {
    // code_1_open_with_hours.json carries hours, as SIGMA can send with no
    // incident.
    let body = text_body(get(fetched("code_1_open_with_hours.json"), "/").await).await;

    assert!(!body.contains("03:00 a 12:00"), "{body}");
    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn a_status_without_details_shows_no_details_list() {
    let body = text_body(get(fetched("blank_strings.json"), "/").await).await;

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
        let body = body_at("code_1_open.json", now, "/").await;

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
        let body = text_body(get_at(latest, fetched_at.parse().unwrap(), "/").await).await;

        assert!(
            body.contains(&format!(
                r#"<time datetime="{fetched_at}">{expected} (hora de Madrid)</time>"#
            )),
            "{fetched_at}: {body}"
        );
    }
}

#[tokio::test]
async fn a_fresh_status_says_when_it_was_last_checked_under_the_verdict_in_madrid_time() {
    let cases = [
        (
            "/",
            r#"<p class="now">Cierra a las 22:00</p><p class="checked">Última comprobación a las 11:30</p><section class="alerts">"#,
        ),
        (
            "/en",
            r#"<p class="now">Closes at 22:00</p><p class="checked">Last checked at 11:30</p><section class="alerts">"#,
        ),
    ];
    for (uri, expected) in cases {
        let body = body_at("code_1_open.json", WITHIN_HOURS, uri).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_check_time_is_in_madrid_time_in_winter() {
    let latest = Ok(Fetched {
        status: sigma::decode(&fixture("code_1_open.json")).unwrap(),
        fetched_at: "2026-11-02T09:30:00Z".parse().unwrap(),
    });

    let body = text_body(get_at(latest, "2026-11-02T09:31:00Z".parse().unwrap(), "/").await).await;

    assert!(
        body.contains(r#"<p class="checked">Última comprobación a las 10:30</p>"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_check_on_an_earlier_day_says_its_date() {
    let cases = [
        (
            "/",
            r#"<p class="checked">Última comprobación el 10 de octubre a las 21:30</p>"#,
        ),
        (
            "/en",
            r#"<p class="checked">Last checked on 10 Oct at 21:30</p>"#,
        ),
    ];
    for (uri, expected) in cases {
        let latest = Ok(Fetched {
            status: sigma::decode(&fixture("code_1_open.json")).unwrap(),
            fetched_at: "2026-10-10T19:30:00Z".parse().unwrap(),
        });

        let body = text_body(get_at(latest, WITHIN_HOURS.parse().unwrap(), uri).await).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn a_stale_status_is_marked_above_the_answer_and_points_to_the_source_under_it() {
    let body = body_at("code_1_open.json", "2026-10-11T09:40:00Z", "/").await;

    for expected in [
        r#"<main><p class="last-known">Último estado conocido</p><h1"#,
        r#"<p class="checked">Última comprobación a las 11:30</p><p class="stale">Puede que este estado no esté al día. <a href="https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&amp;vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&amp;vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&amp;vgnextlocale=es">Consulta la web del Ayuntamiento</a></p><section class="alerts">"#,
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn a_stale_status_is_marked_in_english() {
    let body = body_at("code_1_open.json", "2026-10-11T09:40:00Z", "/en").await;

    for expected in [
        r#"<main><p class="last-known">Last known status</p><h1"#,
        r#"<p class="stale">This status may be out of date. <a href="https://www.madrid.es/"#,
        r#"">Check the City Council’s page</a></p><section class="alerts">"#,
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn a_stale_status_leaves_the_alerts_card_without_a_stale_warning() {
    let body = body_at("code_1_open.json", "2026-10-11T09:40:00Z", "/").await;

    let alerts_card = body
        .split(r#"<section class="alerts">"#)
        .nth(1)
        .and_then(|rest| rest.split("</section>").next())
        .unwrap();
    assert!(
        !alerts_card.contains(r#"<p class="stale">"#),
        "{alerts_card}"
    );
}

#[tokio::test]
async fn a_fresh_status_is_not_marked_as_last_known() {
    let body = body_at("code_1_open.json", "2026-10-11T09:39:59Z", "/").await;

    assert!(!body.contains(r#"<p class="last-known">"#), "{body}");
    assert!(!body.contains(r#"<p class="stale">"#), "{body}");
}

#[tokio::test]
async fn a_failing_upstream_shows_no_check_line() {
    for uri in ["/", "/en"] {
        let body = text_body(get(Err(sigma::FetchError::Timeout), uri).await).await;

        assert!(!body.contains(r#"<p class="checked">"#), "{uri}: {body}");
        assert!(!body.contains(r#"<p class="last-known">"#), "{uri}: {body}");
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
        let body = body_at("code_1_open.json", now, "/").await;

        assert_eq!(body.contains(stale), flagged, "now {now}: {body}");
    }
}

#[tokio::test]
async fn the_status_details_are_shown_in_english() {
    let body = body_at("code_7_closed.json", "2026-10-11T12:00:00Z", "/en").await;

    for expected in [
        "<dt>Incident dates</dt><dd>13 Oct 2026</dd>",
        "<dt>Reopening</dt><dd>See the notes</dd>",
        "<dt>Notes (City Council’s text, in Spanish)</dt><dd>El parque permanecerá cerrado hasta la retirada de las ramas caídas.</dd>",
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
        body.contains("<dt>Incident hours</dt><dd>14:00 to 20:00</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn a_date_range_and_hours_are_translated_in_english() {
    let body = text_body(get(fetched("code_4_orange_alert.json"), "/en").await).await;

    for expected in [
        "<dt>Incident dates</dt><dd>11 Oct 2026 – 12 Oct 2026</dd>",
        "<dt>Incident hours</dt><dd>10:00 to 22:00</dd>",
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn a_live_date_and_hours_ending_at_midnight_are_translated_in_english() {
    let body = text_body(get(fetched("code_6_live_closure.json"), "/en").await).await;

    for expected in [
        "<dt>Incident dates</dt><dd>11 Nov 2026</dd>",
        "<dt>Incident hours</dt><dd>12:00 to midnight</dd>",
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn dates_and_hours_in_an_unrecognised_format_are_shown_as_sigma_wrote_them_in_english() {
    let cases = [
        ("del 11 al 12 de octubre", "por la tarde"),
        ("31/02/2026", "25:00-26:00"),
        ("12/10/2026 - 11/10/2026", "de 10:00 a 22:00"),
        ("1/10/2026", "10:00 a 22:00 h"),
    ];
    for (dates, hours) in cases {
        let latest = Ok(Fetched {
            status: ParkStatus {
                code: AlertCode::OrangeAlert,
                dates: Some(dates.to_owned()),
                hours: Some(hours.to_owned()),
                reopening: None,
                notes: None,
            },
            fetched_at: FETCHED_AT.parse().unwrap(),
        });

        let body = text_body(get(latest, "/en").await).await;

        for expected in [
            format!("<dt>Incident dates</dt><dd>{dates}</dd>"),
            format!("<dt>Incident hours</dt><dd>{hours}</dd>"),
        ] {
            assert!(body.contains(&expected), "{expected}: {body}");
        }
    }
}

fn orange_alert_on(dates: &str, hours: &str) -> Latest {
    Ok(Fetched {
        status: ParkStatus {
            code: AlertCode::OrangeAlert,
            dates: Some(dates.to_owned()),
            hours: Some(hours.to_owned()),
            reopening: None,
            notes: None,
        },
        fetched_at: FETCHED_AT.parse().unwrap(),
    })
}

#[tokio::test]
async fn hours_ending_past_midnight_are_shown_as_sigma_wrote_them_in_english() {
    let latest = orange_alert_on("11/10/2026", "12:00-24:30");

    let body = text_body(get(latest, "/en").await).await;

    assert!(
        body.contains("<dt>Incident hours</dt><dd>12:00-24:30</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn hours_with_minute_sixty_are_shown_as_sigma_wrote_them_in_english() {
    let latest = orange_alert_on("11/10/2026", "12:60-20:00");

    let body = text_body(get(latest, "/en").await).await;

    assert!(
        body.contains("<dt>Incident hours</dt><dd>12:60-20:00</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn a_range_of_one_day_is_translated_as_a_range_in_english() {
    let latest = orange_alert_on("11/10/2026 - 11/10/2026", "10:00-22:00");

    let body = text_body(get(latest, "/en").await).await;

    assert!(
        body.contains("<dt>Incident dates</dt><dd>11 Oct 2026 – 11 Oct 2026</dd>"),
        "{body}"
    );
}

#[tokio::test]
async fn the_notes_are_labelled_as_the_city_councils_spanish_text_in_english() {
    let body = text_body(get(fetched("code_7_closed.json"), "/en").await).await;

    assert!(
        body.contains("<dt>Notes (City Council’s text, in Spanish)</dt>"),
        "{body}"
    );
}

#[tokio::test]
async fn the_spanish_page_shows_the_dates_and_hours_as_sigma_wrote_them() {
    let cases = [
        (
            "code_4_orange_alert.json",
            "11/10/2026 - 12/10/2026",
            "de 10:00 a 22:00 h",
        ),
        ("code_6_live_closure.json", "11/11/2026", "12:00-24:00"),
    ];
    for (name, dates, hours) in cases {
        let body = text_body(get(fetched(name), "/").await).await;

        for expected in [
            format!("<dt>Fechas de la incidencia</dt><dd>{dates}</dd>"),
            format!("<dt>Horario de la incidencia</dt><dd>{hours}</dd>"),
        ] {
            assert!(
                body.contains(&expected),
                "fixture {name}: {expected}: {body}"
            );
        }
    }
}

#[tokio::test]
async fn each_page_shows_the_parks_hours_in_both_seasons() {
    let cases = [
        (
            "/",
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
            "/",
            concat!(
                r#"</span></p></section><section class="next-hours"><h2>Previsión durante el horario habitual</h2>"#,
                r#"<p class="unavailable">La previsión no está disponible.</p></section>"#,
                r#"<section class="schedule">"#,
            ),
        ),
        (
            "/en",
            concat!(
                r#"</span></p></section><section class="next-hours"><h2>Forecast during usual opening hours</h2>"#,
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
        let spanish = text_body(get_with_bot(latest(), "/").await).await;
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
        for uri in ["/", "/en"] {
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
            "/",
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
    for uri in ["/", "/en"] {
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
        ("/", "https://retiro.example/"),
        ("/en", "https://retiro.example/en"),
    ] {
        for latest in [
            || fetched("code_1_open.json"),
            || Err(sigma::FetchError::Timeout),
        ] {
            let body = text_body(get_with_public_url(latest(), uri).await).await;

            for expected in [
                format!(r#"<link rel="canonical" href="{canonical}">"#),
                r#"<link rel="alternate" hreflang="es" href="https://retiro.example/"><link rel="alternate" hreflang="en" href="https://retiro.example/en"><link rel="alternate" hreflang="x-default" href="https://retiro.example/">"#.to_owned(),
                format!(r#"<meta property="og:url" content="{canonical}">"#),
            ] {
                assert!(body.contains(&expected), "{uri}: {expected}: {body}");
            }
        }
    }
}

#[tokio::test]
async fn the_page_has_no_absolute_links_to_itself_without_a_public_url() {
    for uri in ["/", "/en"] {
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
            "code_3_yellow_alert.json",
            WITHIN_HOURS,
            "¿Está abierto el Retiro? Abierto con restricciones",
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
        let body = body_at(name, now, "/").await;

        let expected = format!(r#"<meta property="og:title" content="{title}">"#);
        assert!(body.contains(&expected), "fixture {name} at {now}: {body}");
    }
}

#[tokio::test]
async fn the_link_preview_is_in_the_pages_language() {
    let spanish = body_at("code_1_open.json", OUTSIDE_HOURS, "/").await;
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

const OG_CARDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/og");

const CARDS: [&str; 14] = [
    "open.es.png",
    "open.en.png",
    "open-with-restrictions.es.png",
    "open-with-restrictions.en.png",
    "open-stay-not-advised.es.png",
    "open-stay-not-advised.en.png",
    "closure-expected.es.png",
    "closure-expected.en.png",
    "closed-by-alert.es.png",
    "closed-by-alert.en.png",
    "closed-by-schedule.es.png",
    "closed-by-schedule.en.png",
    "unknown.es.png",
    "unknown.en.png",
];

async fn bytes_body(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

/// Width and height from a PNG's IHDR, its first chunk.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
    assert_eq!(&bytes[12..16], b"IHDR", "first chunk");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height)
}

#[tokio::test]
async fn each_card_is_served_as_a_png_cached_for_a_year() {
    for card in CARDS {
        let response = get(fetched("code_1_open.json"), &format!("/og/{card}")).await;

        assert_eq!(response.status(), StatusCode::OK, "{card}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "image/png",
            "{card}"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable",
            "{card}"
        );
    }
}

#[tokio::test]
async fn each_card_is_1200_by_630_and_under_300_kb() {
    for card in CARDS {
        let response = get(fetched("code_1_open.json"), &format!("/og/{card}")).await;

        let body = bytes_body(response).await;
        assert_eq!(png_size(&body), (1200, 630), "{card}");
        assert!(body.len() < 300 * 1024, "{card}: {} bytes", body.len());
    }
}

#[tokio::test]
async fn each_card_is_the_file_of_its_name() {
    for card in CARDS {
        let response = get(fetched("code_1_open.json"), &format!("/og/{card}")).await;

        let body = bytes_body(response).await;
        let file = fs::read(format!("{OG_CARDS}/{card}")).unwrap();
        assert!(body == file, "{card} is served as another card");
    }
}

#[test]
fn every_card_file_is_served() {
    let files: BTreeSet<String> = fs::read_dir(OG_CARDS)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".png"))
        .collect();

    assert_eq!(
        files,
        CARDS.map(str::to_owned).into(),
        "card files and cards served"
    );
}

#[tokio::test]
async fn an_unknown_card_name_is_not_found() {
    for uri in [
        "/og/missing.es.png",
        "/og/open.fr.png",
        "/og/open.es",
        "/og/card.html",
        "/og/generate.sh",
        "/og/",
    ] {
        let response = get(fetched("code_1_open.json"), uri).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }
}

/// The content of the page's `<meta property="{property}">`.
fn meta_content<'a>(body: &'a str, property: &str) -> &'a str {
    let start = format!(r#"<meta property="{property}" content=""#);
    let (_, rest) = body
        .split_once(&start)
        .unwrap_or_else(|| panic!("no {property}: {body}"));
    rest.split_once('"').unwrap().0
}

#[tokio::test]
async fn each_page_links_a_large_preview_image_with_a_public_url() {
    for uri in ["/", "/en"] {
        let body = text_body(get_with_public_url(Err(sigma::FetchError::Timeout), uri).await).await;

        for expected in [
            r#"<meta property="og:image" content="https://retiro.example/og/"#,
            r#"<meta property="og:image:width" content="1200">"#,
            r#"<meta property="og:image:height" content="630">"#,
            r#"<meta property="og:image:type" content="image/png">"#,
            r#"<meta property="og:image:alt" content=""#,
            r#"<meta name="twitter:card" content="summary_large_image">"#,
        ] {
            assert!(body.contains(expected), "{uri}: {expected}: {body}");
        }
    }
}

#[tokio::test]
async fn the_page_has_no_preview_image_without_a_public_url() {
    for uri in ["/", "/en"] {
        let body = text_body(get(fetched("code_1_open.json"), uri).await).await;

        assert!(!body.contains("og:image"), "{uri}: {body}");
        assert!(!body.contains("twitter:card"), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_preview_image_shows_the_answer_the_preview_gives() {
    let cases = [
        ("code_1_open.json", WITHIN_HOURS, "/", "open.es.png"),
        ("code_1_open.json", WITHIN_HOURS, "/en", "open.en.png"),
        (
            "code_2_incidents.json",
            WITHIN_HOURS,
            "/",
            "open-with-restrictions.es.png",
        ),
        (
            "code_2_incidents.json",
            WITHIN_HOURS,
            "/en",
            "open-with-restrictions.en.png",
        ),
        (
            "code_3_yellow_alert.json",
            WITHIN_HOURS,
            "/",
            "open-with-restrictions.es.png",
        ),
        (
            "code_3_yellow_alert.json",
            WITHIN_HOURS,
            "/en",
            "open-with-restrictions.en.png",
        ),
        (
            "code_4_orange_alert.json",
            WITHIN_HOURS,
            "/",
            "open-stay-not-advised.es.png",
        ),
        (
            "code_4_orange_alert.json",
            WITHIN_HOURS,
            "/en",
            "open-stay-not-advised.en.png",
        ),
        (
            "code_5_red_alert_closure.json",
            WITHIN_HOURS,
            "/",
            "closure-expected.es.png",
        ),
        (
            "code_5_red_alert_closure.json",
            WITHIN_HOURS,
            "/en",
            "closure-expected.en.png",
        ),
        (
            "code_7_closed.json",
            WITHIN_HOURS,
            "/",
            "closed-by-alert.es.png",
        ),
        (
            "code_7_closed.json",
            OUTSIDE_HOURS,
            "/en",
            "closed-by-alert.en.png",
        ),
        (
            "code_1_open.json",
            OUTSIDE_HOURS,
            "/",
            "closed-by-schedule.es.png",
        ),
        (
            "code_1_open.json",
            OUTSIDE_HOURS,
            "/en",
            "closed-by-schedule.en.png",
        ),
        ("unknown_codes.json", WITHIN_HOURS, "/", "unknown.es.png"),
        ("unknown_codes.json", WITHIN_HOURS, "/en", "unknown.en.png"),
    ];
    for (name, now, uri, card) in cases {
        let response = get_with_public_url_at(fetched(name), now.parse().unwrap(), uri).await;

        let body = text_body(response).await;
        assert_eq!(
            meta_content(&body, "og:image"),
            format!("https://retiro.example/og/{card}"),
            "fixture {name} at {now} on {uri}"
        );
    }
}

#[tokio::test]
async fn the_preview_image_without_the_alerts_shows_the_answer_the_hours_give() {
    let cases = [
        (WITHIN_HOURS, "/", "unknown.es.png"),
        (WITHIN_HOURS, "/en", "unknown.en.png"),
        (OUTSIDE_HOURS, "/", "closed-by-schedule.es.png"),
    ];
    for (now, uri, card) in cases {
        let latest = Err(sigma::FetchError::Timeout);
        let response = get_with_public_url_at(latest, now.parse().unwrap(), uri).await;

        let body = text_body(response).await;
        assert_eq!(
            meta_content(&body, "og:image"),
            format!("https://retiro.example/og/{card}"),
            "timeout at {now} on {uri}"
        );
    }
}

#[tokio::test]
async fn the_preview_image_is_described_by_the_preview_title() {
    for uri in ["/", "/en"] {
        let response = get_with_public_url(fetched("code_7_closed.json"), uri).await;

        let body = text_body(response).await;
        assert_eq!(
            meta_content(&body, "og:image:alt"),
            meta_content(&body, "og:title"),
            "{uri}: og:image:alt and og:title"
        );
    }
}

#[tokio::test]
async fn the_page_links_its_favicon() {
    let body = text_body(get(fetched("code_1_open.json"), "/").await).await;

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
        r#"<xhtml:link rel="alternate" hreflang="es" href="https://retiro.example/"/>"#,
        r#"<xhtml:link rel="alternate" hreflang="en" href="https://retiro.example/en"/>"#,
        r#"<xhtml:link rel="alternate" hreflang="x-default" href="https://retiro.example/"/>"#,
    );
    assert_eq!(
        text_body(response).await,
        format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">"#,
                "<url><loc>https://retiro.example/</loc>{alternates}</url>",
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
    for (uri, name) in [("/", "Parque del Retiro"), ("/en", "Retiro Park")] {
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
        let park = structured_data(&body_at("code_1_open.json", now, "/").await);

        assert_eq!(
            park["openingHoursSpecification"],
            serde_json::json!(expected),
            "at {now}"
        );
    }
}

#[tokio::test]
async fn the_structured_data_leaves_out_closures_for_alerts() {
    let body = body_at("code_7_closed.json", FETCHED_AT, "/").await;
    let park = structured_data(&body);

    assert!(
        park.get("specialOpeningHoursSpecification").is_none(),
        "{park}"
    );
}

#[tokio::test]
async fn the_structured_data_links_the_page_with_a_public_url() {
    for (uri, url) in [
        ("/", "https://retiro.example/"),
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
    let park = structured_data(&text_body(get(fetched("code_1_open.json"), "/").await).await);

    assert!(park.get("url").is_none(), "{park}");
}

#[tokio::test]
async fn each_page_names_the_site_with_a_public_url() {
    for uri in ["/", "/en"] {
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
    let body = text_body(get(fetched("code_1_open.json"), "/").await).await;

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
        "[Status page in Spanish](https://retiro.example/)",
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

/// The open hours in the card, leaving out the keys beside them.
fn hour_columns(body: &str) -> usize {
    let (_, hours) = body.split_once(r#"<ol class="hours""#).unwrap();
    hours.matches(r#"<span class="hour">"#).count()
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

    assert_eq!(hour_columns(&body), 12, "{body}");
    let first = concat!(
        r#"</div><ol class="hours" tabindex="0" aria-label="Hourly forecast">"#,
        r#"<li class="current"><div aria-hidden="true"><span class="day">Today</span><span class="hour">Now</span><span class="icon"><svg "#,
    );
    let first_values = concat!(
        r#"</svg></span><span class="temperature">7°</span>"#,
        r#"<span class="rain">70%</span><span class="gusts">17</span></div><span class="reading">"#,
    );
    let second_values = concat!(
        r#"</svg></span><span class="temperature">11°</span>"#,
        r#"<span class="rain">77%</span><span class="gusts">24</span></div><span class="reading">"#,
    );
    let last_values = concat!(
        r#"</svg></span><span class="temperature">14°</span>"#,
        r#"<span class="rain">46%</span><span class="gusts">6</span></div><span class="reading">"#,
    );
    for expected in [first, first_values, second_values, last_values] {
        assert!(body.contains(expected), "{expected}: {body}");
    }
}

#[tokio::test]
async fn the_rain_and_gusts_rows_are_marked_with_an_umbrella_and_a_gust() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T09:30:00+01:00", "/en").await;

    let keys = concat!(
        r#"<div class="keys" aria-hidden="true"><span class="hour"></span><span class="icon"></span>"#,
        r#"<span class="temperature"></span><span class="rain"><svg "#,
    );
    assert!(body.contains(keys), "{body}");
    assert!(
        body.contains(r#"9-9ZM12 12v6a2 2 0 0 0 4 0"/></svg></span><span class="gusts"><svg "#),
        "{body}"
    );
}

#[tokio::test]
async fn the_card_has_a_legend_in_each_language() {
    let cases = [
        ("/", "Probabilidad de lluvia. Rachas en km/h"),
        ("/en", "Rain chance. Gusts in km/h"),
    ];
    for (uri, legend) in cases {
        let body =
            body_with_forecast(forecast("retiro.json"), "2026-11-02T09:30:00+01:00", uri).await;

        assert!(
            body.contains(&format!(r#"</ol></div><p class="legend">{legend}</p>"#)),
            "{uri}: {body}"
        );
    }
}

#[tokio::test]
async fn the_current_hour_shows_today_above_now_in_each_language() {
    let cases = [
        (
            "/",
            r#"<li class="current"><div aria-hidden="true"><span class="day">Hoy</span><span class="hour">Ahora</span>"#,
        ),
        (
            "/en",
            r#"<li class="current"><div aria-hidden="true"><span class="day">Today</span><span class="hour">Now</span>"#,
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
            r#"aria-label="Hourly forecast"><li class=""><div aria-hidden="true"><span class="day">Today</span><span class="hour">6:00</span>"#
        ),
        "{body}"
    );
}

#[tokio::test]
async fn the_night_shows_as_one_closed_block_before_the_next_days_first_hour_in_each_language() {
    let cases = [
        (
            "/",
            r#"<li class="closed"><span>Cerrado por horario</span><span>22–6</span></li><li class=""><div aria-hidden="true"><span class="day">mar</span><span class="hour">6:00</span>"#,
        ),
        (
            "/en",
            r#"<li class="closed"><span>Closed for the night</span><span>22–6</span></li><li class=""><div aria-hidden="true"><span class="day">Tue</span><span class="hour">6:00</span>"#,
        ),
    ];
    for (uri, expected) in cases {
        let body =
            body_with_forecast(forecast("retiro.json"), "2026-11-02T20:30:00+01:00", uri).await;

        assert!(body.contains(expected), "{uri}: {body}");
    }
}

/// The forecast fixture's hours, moved to the live closure's day, 11/11/2026.
fn forecast_on_the_live_closures_day() -> open_meteo::Latest {
    forecast_moved_by(9)
}

/// The forecast fixture's hours, from 02/11/2026, moved `days` on.
fn forecast_moved_by(days: i64) -> open_meteo::Latest {
    let mut latest = forecast("rain_chances.json");
    for hour in &mut latest.as_mut().unwrap().forecast.hours {
        hour.at = hour.at.checked_add(jiff::ToSpan::days(days)).unwrap();
    }
    latest
}

#[tokio::test]
async fn an_alert_closures_window_is_noted_above_the_forecast() {
    let cases = [
        (
            BEFORE_THE_ALERT,
            "/",
            r#"<h2>Previsión durante el horario habitual</h2><p class="alert-closure">Cierre por alerta de 12:00 a medianoche</p><div class="forecast">"#,
        ),
        (
            "2026-11-11T14:00:00Z",
            "/",
            r#"<h2>Previsión durante el horario habitual</h2><p class="alert-closure">Cierre por alerta de 12:00 a medianoche</p><div class="forecast">"#,
        ),
        (
            BEFORE_THE_ALERT,
            "/en",
            r#"<h2>Forecast during usual opening hours</h2><p class="alert-closure">Alert closure from 12:00 to midnight</p><div class="forecast">"#,
        ),
    ];
    for (now, uri, expected) in cases {
        let body = text_body(
            get_with_forecast(
                fetched("code_6_live_closure.json"),
                forecast_on_the_live_closures_day(),
                now,
                uri,
            )
            .await,
        )
        .await;

        assert!(body.contains(expected), "{now} {uri}: {body}");
    }
}

#[tokio::test]
async fn an_alert_closure_on_a_later_day_has_no_note_above_the_forecast() {
    // 11:30 and 18:00 in Madrid the day before the live capture's window.
    for now in ["2026-11-10T10:30:00Z", "2026-11-10T17:00:00Z"] {
        let body = text_body(
            get_with_forecast(
                fetched("code_6_live_closure.json"),
                forecast_moved_by(8),
                now,
                "/",
            )
            .await,
        )
        .await;

        assert!(
            body.contains(
                r#"<h2>Previsión durante el horario habitual</h2><div class="forecast">"#
            ),
            "at {now}: {body}"
        );
    }
}

#[tokio::test]
async fn an_alert_closure_without_a_readable_window_has_no_note_above_the_forecast() {
    let body = text_body(
        get_with_forecast(
            fetched("code_5_live_forecast_without_hours.json"),
            forecast_on_the_live_closures_day(),
            BEFORE_THE_ALERT,
            "/",
        )
        .await,
    )
    .await;

    assert!(
        body.contains(r#"<h2>Previsión durante el horario habitual</h2><div class="forecast">"#),
        "{body}"
    );
}

#[tokio::test]
async fn the_closed_block_does_not_count_towards_the_twelve_hours() {
    let body =
        body_with_forecast(forecast("retiro.json"), "2026-11-02T20:30:00+01:00", "/en").await;

    assert_eq!(hour_columns(&body), 12, "{body}");
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
            r#"<li class="closed"><span>Closed for the night</span><span>0–6</span></li><li class=""><div aria-hidden="true"><span class="day">Sat</span>"#
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
            r#"aria-label="Hourly forecast"><li class=""><div aria-hidden="true">"#,
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
            r#"<span class="gusts">–</span></div><span class="reading">"#,
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
            r#"<span class="gusts">17</span></div><span class="reading">"#,
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
            r#"<li class="current"><div aria-hidden="true"><span class="day">Today</span><span class="hour">Now</span><span class="icon"></span>"#,
            r#"<span class="temperature"></span><span class="rain"></span>"#,
            r#"<span class="gusts"></span></div><span class="reading">Now</span></li>"#,
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
            r#"<li class="current"><div aria-hidden="true"><span class="day">Today</span><span class="hour">Now</span><span class="icon"></span>"#,
            r#"<span class="temperature"></span><span class="rain"></span>"#,
            r#"<span class="gusts"></span></div><span class="reading">Now</span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn each_hour_reads_its_values_as_one_sentence_in_each_language() {
    let cases = [
        (
            "/",
            "14:00, despejado, 18 grados, 4 % de probabilidad de lluvia, rachas de 16 km/h",
        ),
        (
            "/en",
            "14:00, clear sky, 18 degrees, 4% chance of rain, gusts of 16 km/h",
        ),
    ];
    for (uri, reading) in cases {
        let body = body_with_forecast(
            forecast("rain_chances.json"),
            "2026-11-02T09:30:00+01:00",
            uri,
        )
        .await;

        let expected = format!(r#"<span class="reading">{reading}</span></li>"#);
        assert!(body.contains(&expected), "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_visible_values_are_hidden_from_screen_readers_in_favour_of_the_reading() {
    let body = body_with_forecast(
        forecast_of(hour_at_nine()),
        "2026-11-02T09:30:00+01:00",
        "/en",
    )
    .await;

    assert!(
        body.contains(concat!(
            r#"<li class="current"><div aria-hidden="true"><span class="day">Today</span>"#,
            r#"<span class="hour">Now</span><span class="icon"><svg "#,
        )),
        "{body}"
    );
    assert!(
        body.contains(concat!(
            r#"<span class="gusts">17</span></div>"#,
            r#"<span class="reading">Now, clear sky, 7 degrees, 30% chance of rain, gusts of 17 km/h</span></li>"#,
        )),
        "{body}"
    );
}

#[tokio::test]
async fn the_reading_names_the_day_of_a_column_that_shows_one() {
    let cases = [
        ("2026-11-02T05:30:00+01:00", "/", "hoy 6:00, "),
        ("2026-11-02T05:30:00+01:00", "/en", "today 6:00, "),
        ("2026-11-02T20:30:00+01:00", "/", "martes 6:00, "),
        ("2026-11-02T20:30:00+01:00", "/en", "Tuesday 6:00, "),
    ];
    for (now, uri, when) in cases {
        let body = body_with_forecast(forecast("retiro.json"), now, uri).await;

        let expected = format!(r#"<span class="reading">{when}"#);
        assert!(body.contains(&expected), "{now} {uri}: {body}");
    }
}

#[tokio::test]
async fn a_reading_leaves_out_a_missing_rain_chance_and_gusts() {
    let hour = HourForecast {
        rain_chance: None,
        gusts: None,
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/en").await;

    assert!(
        body.contains(r#"<span class="reading">Now, clear sky, 7 degrees</span>"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_reading_leaves_out_the_name_of_an_unknown_weather_code() {
    let hour = HourForecast {
        weather_code: Some(4),
        ..hour_at_nine()
    };

    let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", "/").await;

    assert!(
        body.contains(
            r#"<span class="reading">Ahora, 7 grados, 30 % de probabilidad de lluvia, rachas de 17 km/h</span>"#
        ),
        "{body}"
    );
}

#[tokio::test]
async fn a_reading_of_one_degree_either_side_of_zero_is_singular() {
    let cases = [
        (1.2, "/", "Ahora, despejado, 1 grado, "),
        (-0.6, "/", "Ahora, despejado, -1 grado, "),
        (1.2, "/en", "Now, clear sky, 1 degree, "),
        (-0.6, "/en", "Now, clear sky, -1 degree, "),
    ];
    for (temperature, uri, reading) in cases {
        let hour = HourForecast {
            temperature: Some(temperature),
            ..hour_at_nine()
        };

        let body = body_with_forecast(forecast_of(hour), "2026-11-02T09:30:00+01:00", uri).await;

        let expected = format!(r#"<span class="reading">{reading}"#);
        assert!(body.contains(&expected), "{temperature} {uri}: {body}");
    }
}

const WEATHER_NAMES: [(u8, &str, &str); 28] = [
    (0, "despejado", "clear sky"),
    (1, "mayormente despejado", "mainly clear"),
    (2, "parcialmente nublado", "partly cloudy"),
    (3, "cubierto", "overcast"),
    (45, "niebla", "fog"),
    (48, "niebla con escarcha", "rime fog"),
    (51, "llovizna ligera", "light drizzle"),
    (53, "llovizna moderada", "moderate drizzle"),
    (55, "llovizna intensa", "dense drizzle"),
    (56, "llovizna helada ligera", "light freezing drizzle"),
    (57, "llovizna helada intensa", "dense freezing drizzle"),
    (61, "lluvia ligera", "slight rain"),
    (63, "lluvia moderada", "moderate rain"),
    (65, "lluvia intensa", "heavy rain"),
    (66, "lluvia helada ligera", "light freezing rain"),
    (67, "lluvia helada intensa", "heavy freezing rain"),
    (71, "nevada ligera", "slight snowfall"),
    (73, "nevada moderada", "moderate snowfall"),
    (75, "nevada intensa", "heavy snowfall"),
    (77, "granos de nieve", "snow grains"),
    (80, "chubascos ligeros", "slight rain showers"),
    (81, "chubascos moderados", "moderate rain showers"),
    (82, "chubascos violentos", "violent rain showers"),
    (85, "chubascos de nieve ligeros", "slight snow showers"),
    (86, "chubascos de nieve intensos", "heavy snow showers"),
    (95, "tormenta", "thunderstorm"),
    (
        96,
        "tormenta con granizo ligero",
        "thunderstorm with slight hail",
    ),
    (
        99,
        "tormenta con granizo intenso",
        "thunderstorm with heavy hail",
    ),
];

#[tokio::test]
async fn each_weather_code_is_read_by_its_name_in_each_language() {
    for (code, spanish, english) in WEATHER_NAMES {
        let hour = HourForecast {
            weather_code: Some(code),
            ..hour_at_nine()
        };
        let forecast = forecast_of(hour);

        let body_es = body_with_forecast(forecast.clone(), "2026-11-02T09:30:00+01:00", "/").await;
        let body_en = body_with_forecast(forecast, "2026-11-02T09:30:00+01:00", "/en").await;

        let expected_es = format!(r#"<span class="reading">Ahora, {spanish}, 7 grados"#);
        let expected_en = format!(r#"<span class="reading">Now, {english}, 7 degrees"#);
        assert!(body_es.contains(&expected_es), "code {code}: {body_es}");
        assert!(body_en.contains(&expected_en), "code {code}: {body_en}");
    }
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
    let body = body_with_forecast(forecast("retiro.json"), "2026-11-04T00:00:00+01:00", "/").await;

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
    let body = body_with_forecast(forecast("retiro.json"), "2026-11-02T14:15:00+01:00", "/").await;

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

    assert_eq!(hour_columns(&body), 4, "{body}");
}

#[tokio::test]
async fn the_status_is_the_same_with_and_without_a_forecast() {
    let now = "2026-11-02T09:30:00+01:00";
    let open = get_with_forecast(fetched("code_1_open.json"), None, now, "/").await;
    let open_with_forecast = get_with_forecast(
        fetched("code_1_open.json"),
        forecast("retiro.json"),
        now,
        "/",
    )
    .await;

    assert_eq!(open.status(), StatusCode::OK);
    assert_eq!(open_with_forecast.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_failing_status_keeps_its_gateway_status_with_and_without_a_forecast() {
    let now = "2026-11-02T09:30:00+01:00";
    let without = get_with_forecast(Err(sigma::FetchError::Timeout), None, now, "/").await;
    let with = get_with_forecast(
        Err(sigma::FetchError::Timeout),
        forecast("retiro.json"),
        now,
        "/",
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
            "/",
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
