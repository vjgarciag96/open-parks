use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use open_parks::page;
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma;
use tokio::sync::watch;
use tower::ServiceExt;

mod common;
use common::fixture;

async fn get(latest: Latest, uri: &str) -> Response {
    page::router(watch::channel(latest).1)
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn fetched(fixture_name: &str) -> Latest {
    Ok(Fetched {
        status: sigma::decode(&fixture(fixture_name)).unwrap(),
        fetched_at: "2026-10-11T09:30:00Z".parse().unwrap(),
    })
}

fn assert_html(response: &Response) {
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
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
        let body = text_body(get(fetched(name), "/").await).await;

        assert!(
            body.contains(&format!("<h1>{headline}</h1>")),
            "fixture {name}: {body}"
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
        body.contains("<h1>No hemos podido consultar el estado</h1>"),
        "{body}"
    );
}

#[tokio::test]
async fn an_upstream_that_never_responds_is_a_gateway_timeout_page() {
    let response = get(Err(sigma::FetchError::Timeout), "/").await;

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = text_body(response).await;
    assert!(
        body.contains("<h1>No hemos podido consultar el estado</h1>"),
        "{body}"
    );
}

#[tokio::test]
async fn the_page_has_no_scripts_and_inline_styles_only() {
    let body = text_body(get(fetched("code_1_open.json"), "/").await).await;

    assert!(!body.contains("<script"), "{body}");
    assert!(!body.contains("<link"), "{body}");
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
