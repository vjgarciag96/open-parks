use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::i18n::Lang;
use crate::park::headline::{Alerts, Answer, Headline};
use crate::park::status::Verdict;

pub(super) const WIDTH: u32 = 1200;
pub(super) const HEIGHT: u32 = 630;

/// Names each card once: its slug picks its file in each language, and
/// `include_bytes!` only takes a literal path.
macro_rules! cards {
    ($($card:ident => $slug:literal,)*) => {
        #[derive(Clone, Copy)]
        enum Card {
            $($card,)*
        }

        impl Card {
            const ALL: &[Card] = &[$(Card::$card,)*];

            fn slug(self) -> &'static str {
                match self {
                    $(Card::$card => $slug,)*
                }
            }

            fn png(self, lang: Lang) -> &'static [u8] {
                match (self, lang) {
                    $(
                        (Card::$card, Lang::Es) => {
                            include_bytes!(concat!("../../../assets/og/", $slug, ".es.png"))
                        }
                        (Card::$card, Lang::En) => {
                            include_bytes!(concat!("../../../assets/og/", $slug, ".en.png"))
                        }
                    )*
                }
            }
        }
    };
}

cards! {
    Open => "open",
    OpenWithRestrictions => "open-with-restrictions",
    OpenStayNotAdvised => "open-stay-not-advised",
    ClosureExpected => "closure-expected",
    ClosedByAlert => "closed-by-alert",
    ClosedBySchedule => "closed-by-schedule",
    Unknown => "unknown",
}

/// The URL path of the card for the page's link preview.
pub(super) fn path(headline: &Headline, lang: Lang) -> String {
    format!("/og/{}", file_name(card(headline), lang))
}

fn file_name(card: Card, lang: Lang) -> String {
    format!("{}.{}.png", card.slug(), lang.code())
}

/// The card for the answer a link preview gives, with the alert's
/// restrictions when the park is open, as the page's alert line gives them.
fn card(headline: &Headline) -> Card {
    match (headline.preview, headline.alerts) {
        (Answer::Open, Alerts::Checked(Verdict::OpenWithRestrictions)) => {
            Card::OpenWithRestrictions
        }
        (Answer::Open, Alerts::Checked(Verdict::OpenStayNotAdvised)) => Card::OpenStayNotAdvised,
        (Answer::Open, _) => Card::Open,
        (Answer::OpenClosureExpected, _) => Card::ClosureExpected,
        (Answer::ClosedByAlert, _) => Card::ClosedByAlert,
        (Answer::ClosedBySchedule, _) => Card::ClosedBySchedule,
        (Answer::Unknown | Answer::Unchecked, _) => Card::Unknown,
        // A preview always names the cause of a closure.
        (Answer::Closed, _) => Card::ClosedByAlert,
    }
}

/// The card named `file`, cached a year: a redesign gets a new name.
pub(super) fn response(file: &str) -> Response {
    let png = Card::ALL
        .iter()
        .flat_map(|&card| Lang::ALL.map(|lang| (card, lang)))
        .find(|&(card, lang)| file_name(card, lang) == file)
        .map(|(card, lang)| card.png(lang));
    match png {
        Some(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            png,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
