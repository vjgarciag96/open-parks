use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use open_parks::{api, sigma};
use serde_json::{Value, json};
use tower::ServiceExt;

mod common;
use common::fixture;

async fn get(fixture_name: &str, uri: &str) -> axum::response::Response {
    let status = sigma::decode(&fixture(fixture_name)).unwrap();
    api::router(status)
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn get_retiro_status(fixture_name: &str) -> axum::response::Response {
    get(fixture_name, "/api/v1/parks/retiro/status").await
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn retiro_status_is_served_as_json() {
    let response = get_retiro_status("code_1_open.json").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
}

#[tokio::test]
async fn retiro_status_is_served_as_machine_values_with_the_raw_details() {
    let body = json_body(get_retiro_status("code_6_red_alert_restrictions.json").await).await;

    assert_eq!(
        body,
        json!({
            "code": "red_alert_restrictions",
            "verdict": "open_with_restrictions",
            "dates": "12/10/2026",
            "hours": "de 14:00 a 20:00 h",
            "reopening": null,
            "notes": "Cerradas las zonas infantiles, deportivas y de mayores. \
                      Suspendidos los eventos al aire libre.",
        })
    );
}

#[tokio::test]
async fn every_alert_code_is_served_by_name_with_its_verdict() {
    let cases = [
        ("code_1_open.json", "open", "open"),
        (
            "code_2_incidents.json",
            "incidents",
            "open_with_restrictions",
        ),
        (
            "code_3_yellow_alert.json",
            "yellow_alert",
            "open_with_restrictions",
        ),
        (
            "code_4_orange_alert.json",
            "orange_alert",
            "open_stay_not_advised",
        ),
        (
            "code_5_red_alert_closure.json",
            "red_alert_closure",
            "closure_forecast",
        ),
        (
            "code_6_red_alert_restrictions.json",
            "red_alert_restrictions",
            "open_with_restrictions",
        ),
        ("code_7_closed.json", "closed", "closed"),
        ("unknown_codes.json", "unknown", "unknown"),
    ];
    for (name, code, verdict) in cases {
        let body = json_body(get_retiro_status(name).await).await;

        assert_eq!(
            (&body["code"], &body["verdict"]),
            (&json!(code), &json!(verdict)),
            "fixture {name}"
        );
    }
}

#[tokio::test]
async fn every_reopening_forecast_is_served_by_name() {
    let cases = [
        ("code_5_red_alert_closure.json", "after_inspections"),
        ("code_7_closed.json", "see_notes"),
        ("unknown_codes.json", "unknown"),
    ];
    for (name, reopening) in cases {
        let body = json_body(get_retiro_status(name).await).await;

        assert_eq!(body["reopening"], json!(reopening), "fixture {name}");
    }
}

#[tokio::test]
async fn absent_details_are_served_as_null() {
    let body = json_body(get_retiro_status("blank_strings.json").await).await;

    assert_eq!(
        body,
        json!({
            "code": "open",
            "verdict": "open",
            "dates": null,
            "hours": null,
            "reopening": null,
            "notes": null,
        })
    );
}

#[tokio::test]
async fn other_parks_are_not_found() {
    let response = get("code_1_open.json", "/api/v1/parks/casa-de-campo/status").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
