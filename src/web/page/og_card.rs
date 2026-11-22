use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

const CARDS: [(&str, &[u8]); 10] = [
    (
        "open.es.png",
        include_bytes!("../../../assets/og/open.es.png"),
    ),
    (
        "open.en.png",
        include_bytes!("../../../assets/og/open.en.png"),
    ),
    (
        "closure-expected.es.png",
        include_bytes!("../../../assets/og/closure-expected.es.png"),
    ),
    (
        "closure-expected.en.png",
        include_bytes!("../../../assets/og/closure-expected.en.png"),
    ),
    (
        "closed-by-alert.es.png",
        include_bytes!("../../../assets/og/closed-by-alert.es.png"),
    ),
    (
        "closed-by-alert.en.png",
        include_bytes!("../../../assets/og/closed-by-alert.en.png"),
    ),
    (
        "closed-by-schedule.es.png",
        include_bytes!("../../../assets/og/closed-by-schedule.es.png"),
    ),
    (
        "closed-by-schedule.en.png",
        include_bytes!("../../../assets/og/closed-by-schedule.en.png"),
    ),
    (
        "unknown.es.png",
        include_bytes!("../../../assets/og/unknown.es.png"),
    ),
    (
        "unknown.en.png",
        include_bytes!("../../../assets/og/unknown.en.png"),
    ),
];

/// The card named `file`, cached a year: a redesign gets a new name.
pub(super) fn response(file: &str) -> Response {
    match CARDS.iter().find(|(name, _)| *name == file) {
        Some((_, png)) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            *png,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
