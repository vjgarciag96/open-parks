use jiff::Timestamp;
use serde_json::{Value, json};

use crate::i18n::Lang;
use crate::park::schedule::{self, Dated};
use crate::web::public_url::PublicUrl;

// The name, address and coordinates are the Ayuntamiento's, from the page
// linked in park/schedule.rs; the Wikidata item is Q1131807, "El Retiro Park".
pub(crate) const SITE_NAME: &str = "Retiro Abierto";
const OFFICIAL_NAME: &str = "Jardines de El Buen Retiro";
const WIKIDATA: &str = "https://www.wikidata.org/wiki/Q1131807";
const LATITUDE: f64 = 40.41964087412762;
const LONGITUDE: f64 = -3.68793370943788;

const EVERY_DAY: [&str; 7] = [
    "https://schema.org/Monday",
    "https://schema.org/Tuesday",
    "https://schema.org/Wednesday",
    "https://schema.org/Thursday",
    "https://schema.org/Friday",
    "https://schema.org/Saturday",
    "https://schema.org/Sunday",
];

/// The park as schema.org JSON-LD, for a `<script type="application/ld+json">`.
///
/// It gives the regular hours only, not closures for alerts: search engines
/// crawl too seldom to keep up with those, and would go on showing a closure
/// long after the park reopened.
pub(crate) fn park(lang: Lang, public_url: Option<&PublicUrl>, now: Timestamp) -> String {
    let mut park = json!({
        "@context": "https://schema.org",
        "@type": "Park",
        "name": lang.strings().park_name,
        "alternateName": OFFICIAL_NAME,
        "sameAs": WIKIDATA,
        "address": {
            "@type": "PostalAddress",
            "streetAddress": "Plaza de la Independencia, 7",
            "postalCode": "28001",
            "addressLocality": "Madrid",
            "addressCountry": "ES",
        },
        "geo": {
            "@type": "GeoCoordinates",
            "latitude": LATITUDE,
            "longitude": LONGITUDE,
        },
        "openingHoursSpecification": schedule::upcoming_seasons(now).map(hours),
    });
    if let Some(public_url) = public_url {
        park["url"] = public_url.of(lang.path()).into();
    }
    escaped(park)
}

/// The site as schema.org JSON-LD, for the name search results show above the
/// title. It's on every page because the home page only redirects to them.
pub(crate) fn website(public_url: &PublicUrl) -> String {
    escaped(json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": SITE_NAME,
        "url": public_url.of("/"),
    }))
}

fn escaped(data: Value) -> String {
    // `<` can't end the script once escaped, and JSON reads it the same.
    data.to_string().replace('<', "\\u003c")
}

fn hours(dated: Dated) -> Value {
    json!({
        "@type": "OpeningHoursSpecification",
        "dayOfWeek": EVERY_DAY,
        "opens": hour(dated.season.opens),
        "closes": hour(dated.season.closes),
        "validFrom": dated.first_day.to_string(),
        "validThrough": dated.last_day.to_string(),
    })
}

/// Closing at 24 is written 00:00: schema.org has no 24:00, and a closing
/// before the opening runs past midnight.
fn hour(hour: i8) -> String {
    format!("{:02}:00", hour % 24)
}
