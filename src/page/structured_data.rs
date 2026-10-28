use jiff::Timestamp;
use jiff::civil::Date;
use maud::{Markup, PreEscaped, html};
use serde_json::{Value, json};

use crate::lang::Lang;
use crate::public_url::PublicUrl;
use crate::schedule::{SEASONS, Season};

use super::SOURCE_URL;

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// schema.org JSON-LD for search engines and AI agents: the page, and the
/// park it's about with its hours for the season under way and the next.
pub(super) fn markup(
    lang: Lang,
    public_url: Option<&PublicUrl>,
    fetched_at: Option<Timestamp>,
    today: Date,
) -> Markup {
    let strings = lang.strings();
    let mut page = json!({
        "@context": "https://schema.org",
        "@type": "WebPage",
        "name": strings.title,
        "description": strings.description,
        "inLanguage": lang.code(),
        "isBasedOn": SOURCE_URL,
        "about": {
            "@type": "Park",
            "name": strings.park,
            "alternateName": "Jardines del Buen Retiro",
            "address": {
                "@type": "PostalAddress",
                "addressLocality": "Madrid",
                "addressCountry": "ES",
            },
            "geo": {
                "@type": "GeoCoordinates",
                "latitude": 40.4153,
                "longitude": -3.6845,
            },
            "openingHoursSpecification": SEASONS
                .iter()
                .map(|season| hours(season, today))
                .collect::<Vec<_>>(),
        },
    });
    if let Some(public_url) = public_url {
        page["url"] = public_url.of(lang.path()).into();
    }
    if let Some(fetched_at) = fetched_at {
        page["dateModified"] = fetched_at.to_string().into();
    }
    // JSON can't end the script element early unless it holds "</".
    let json = page.to_string().replace("</", r"<\/");
    html! {
        script type="application/ld+json" { (PreEscaped(json)) }
    }
}

fn hours(season: &Season, today: Date) -> Value {
    let (from, through) = season.run_from(today);
    json!({
        "@type": "OpeningHoursSpecification",
        "dayOfWeek": DAYS,
        "opens": format!("{:02}:00", season.opens),
        // schema.org times are within a day, so midnight is the day's last
        // minute.
        "closes": match season.closes {
            24 => "23:59".to_owned(),
            closes => format!("{closes:02}:00"),
        },
        "validFrom": from.to_string(),
        "validThrough": through.to_string(),
    })
}
