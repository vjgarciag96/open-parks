use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use jiff::Timestamp;
use open_parks::page;
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma;
use reqwest::Url;
use tokio::sync::watch;
use tower::ServiceExt;

mod common;
use common::fixture;

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

fn public(url: &str) -> page::PublicUrl {
    page::PublicUrl::new(Url::parse(url).unwrap()).unwrap()
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
    format!(r#"<h1><span class="question">{question}</span> {answer}</h1>"#)
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

#[tokio::test]
async fn every_alert_codes_verdict_is_the_headline() {
    let cases = [
        ("code_1_open.json", "Abierto"),
        ("code_2_incidents.json", "Abierto con restricciones"),
        ("code_3_yellow_alert.json", "Abierto con restricciones"),
        (
            "code_4_orange_alert.json",
            "Abierto, pero no se recomienda permanecer",
        ),
        ("code_5_red_alert_closure.json", "Cierre previsto"),
        (
            "code_6_red_alert_restrictions.json",
            "Abierto con restricciones",
        ),
        ("code_7_closed.json", "Cerrado"),
        ("unknown_codes.json", "Estado desconocido"),
    ];
    for (name, headline) in cases {
        let body = text_body(get(fetched(name), "/es").await).await;

        assert!(
            body.contains(&h1("¿Está abierto el Retiro?", headline)),
            "fixture {name}: {body}"
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
async fn every_alert_codes_verdict_is_the_english_headline() {
    let cases = [
        ("code_1_open.json", "Open"),
        ("code_2_incidents.json", "Open with restrictions"),
        ("code_3_yellow_alert.json", "Open with restrictions"),
        (
            "code_4_orange_alert.json",
            "Open, but staying is not advised",
        ),
        ("code_5_red_alert_closure.json", "Closure expected"),
        (
            "code_6_red_alert_restrictions.json",
            "Open with restrictions",
        ),
        ("code_7_closed.json", "Closed"),
        ("unknown_codes.json", "Status unknown"),
    ];
    for (name, headline) in cases {
        let response = get(fetched(name), "/en").await;

        assert_eq!(response.status(), StatusCode::OK, "fixture {name}");
        let body = text_body(response).await;
        assert!(
            body.contains(r#"<html lang="en">"#),
            "fixture {name}: {body}"
        );
        assert!(
            body.contains(&h1("Is Retiro Park open?", headline)),
            "fixture {name}: {body}"
        );
    }
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
        body.contains("<dt>Horario</dt><dd>de 14:00 a 20:00 h</dd>"),
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
    for term in ["Horario", "Reapertura", "Observaciones"] {
        assert!(!body.contains(term), "{term}: {body}");
    }
}

#[tokio::test]
async fn a_status_without_details_shows_no_details_list() {
    let body = text_body(get(fetched("blank_strings.json"), "/es").await).await;

    assert!(!body.contains("<dl>"), "{body}");
}

#[tokio::test]
async fn the_page_says_how_long_ago_the_status_was_fetched() {
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
                r#"Actualizado <time datetime="2026-10-11T09:30:00Z">{ago}</time>"#
            )),
            "now {now}: {body}"
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
        r#"Updated <time datetime="2026-10-11T09:30:00Z">2 hours ago</time>"#,
        "This status may be out of date.",
    ] {
        assert!(body.contains(expected), "{expected}: {body}");
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
            "<title>¿Está abierto el Retiro hoy? Parque del Retiro, Madrid</title>",
            r#"<meta name="description" content="Consulta si el Parque del Retiro de Madrid está abierto"#,
        ),
        (
            "/en",
            "<title>Is Retiro Park open today? Live status, Madrid</title>",
            r#"<meta name="description" content="Check whether Madrid’s Retiro Park is open"#,
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
async fn the_link_preview_shows_the_verdict() {
    let spanish = text_body(get(fetched("code_7_closed.json"), "/es").await).await;
    let english = text_body(get(fetched("code_7_closed.json"), "/en").await).await;

    for expected in [
        r#"<meta property="og:type" content="website">"#,
        r#"<meta property="og:title" content="¿Está abierto el Retiro? Cerrado">"#,
        r#"<meta property="og:description" content="Consulta si el Parque del Retiro"#,
        r#"<meta property="og:locale" content="es_ES"><meta property="og:locale:alternate" content="en_GB">"#,
    ] {
        assert!(spanish.contains(expected), "{expected}: {spanish}");
    }
    for expected in [
        r#"<meta property="og:title" content="Is Retiro Park open? Closed">"#,
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

#[test]
fn a_public_url_is_an_http_or_https_origin() {
    for url in [
        "https://retiro.example",
        "https://retiro.example/",
        "http://localhost:3000",
    ] {
        assert!(
            page::PublicUrl::new(Url::parse(url).unwrap()).is_some(),
            "{url}"
        );
    }
    for url in [
        "https://retiro.example/parks",
        "https://retiro.example/?lang=es",
        "https://retiro.example/#top",
        "https://user@retiro.example",
        "ftp://retiro.example",
        "mailto:retiro@example.org",
    ] {
        assert!(
            page::PublicUrl::new(Url::parse(url).unwrap()).is_none(),
            "{url}"
        );
    }
}
