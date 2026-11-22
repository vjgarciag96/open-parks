use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::i18n::Lang;
use crate::park::headline::{Alerts, Answer, Headline};
use crate::park::status::Verdict;

pub(super) const WIDTH: u32 = 1200;
pub(super) const HEIGHT: u32 = 630;

const CARDS: [(&str, &[u8]); 14] = [
    (
        "open.es.png",
        include_bytes!("../../../assets/og/open.es.png"),
    ),
    (
        "open.en.png",
        include_bytes!("../../../assets/og/open.en.png"),
    ),
    (
        "open-with-restrictions.es.png",
        include_bytes!("../../../assets/og/open-with-restrictions.es.png"),
    ),
    (
        "open-with-restrictions.en.png",
        include_bytes!("../../../assets/og/open-with-restrictions.en.png"),
    ),
    (
        "open-stay-not-advised.es.png",
        include_bytes!("../../../assets/og/open-stay-not-advised.es.png"),
    ),
    (
        "open-stay-not-advised.en.png",
        include_bytes!("../../../assets/og/open-stay-not-advised.en.png"),
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

/// The card for the answer a link preview gives, with the alert's
/// restrictions when the park is open, as the page's alert line gives them.
pub(super) fn path(headline: &Headline, lang: Lang) -> String {
    format!("/og/{}.{}.png", slug(headline), lang.code())
}

fn slug(headline: &Headline) -> &'static str {
    match (headline.preview, headline.alerts) {
        (Answer::Open, Alerts::Checked(Verdict::OpenWithRestrictions)) => "open-with-restrictions",
        (Answer::Open, Alerts::Checked(Verdict::OpenStayNotAdvised)) => "open-stay-not-advised",
        (Answer::Open, _) => "open",
        (Answer::OpenClosureExpected, _) => "closure-expected",
        (Answer::ClosedByAlert, _) => "closed-by-alert",
        (Answer::ClosedBySchedule, _) => "closed-by-schedule",
        (Answer::Unknown | Answer::Unchecked, _) => "unknown",
        // A preview always names the cause of a closure.
        (Answer::Closed, _) => "closed-by-alert",
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
