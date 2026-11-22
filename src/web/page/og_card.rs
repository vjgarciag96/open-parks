use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::i18n::Lang;
use crate::park::headline::Answer;

pub(super) const WIDTH: u32 = 1200;
pub(super) const HEIGHT: u32 = 630;

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

/// The card for the answer a link preview gives.
pub(super) fn path(answer: Answer, lang: Lang) -> String {
    format!("/og/{}.{}.png", slug(answer), lang.code())
}

fn slug(answer: Answer) -> &'static str {
    match answer {
        Answer::Open => "open",
        Answer::OpenClosureExpected => "closure-expected",
        Answer::ClosedByAlert => "closed-by-alert",
        Answer::ClosedBySchedule => "closed-by-schedule",
        Answer::Unknown | Answer::Unchecked => "unknown",
        // A preview always names the cause of a closure.
        Answer::Closed => "closed-by-alert",
    }
}

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
