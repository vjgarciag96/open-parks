use jiff::{SignedDuration, Timestamp};
use maud::{Markup, html};

use crate::i18n::{Age, Strings};
use crate::open_meteo;
use crate::park::forecast::HourForecast;

/// A forecast past this age still shows, with a note saying how old it is.
const OLD_AFTER: SignedDuration = SignedDuration::from_hours(3);

pub(super) fn card(strings: &Strings, forecast: &open_meteo::Latest, now: Timestamp) -> Markup {
    let hours = forecast
        .as_ref()
        .map_or(&[][..], |fetched| fetched.forecast.next_hours(now));
    html! {
        section.next-hours {
            h2 { (strings.next_hours) }
            @if let (Some(fetched), false) = (forecast, hours.is_empty()) {
                ol.hours tabindex="0" aria-label=(strings.hourly_forecast) {
                    @for (index, hour) in hours.iter().enumerate() {
                        (column(strings, hour, index == 0))
                    }
                }
                p.legend { (strings.forecast_legend) }
                (age_note(strings, now.duration_since(fetched.fetched_at)))
            } @else {
                p.unavailable { (strings.forecast_unavailable) }
            }
        }
    }
}

fn column(strings: &Strings, hour: &HourForecast, current: bool) -> Markup {
    html! {
        li.current[current] {
            span.hour {
                @if current {
                    (strings.hour_now)
                } @else {
                    (hour.at.hour()) ":00"
                }
            }
            span.temperature {
                @if let Some(temperature) = hour.temperature {
                    (rounded(temperature)) "°"
                }
            }
            span.rain {
                @if let Some(rain_chance) = hour.rain_chance {
                    (rain_chance) "%"
                }
            }
            span.gusts {
                @if let Some(gusts) = hour.gusts {
                    (rounded(gusts))
                }
            }
        }
    }
}

fn age_note(strings: &Strings, elapsed: SignedDuration) -> Markup {
    html! {
        @if elapsed > OLD_AFTER {
            p.forecast-age { (strings.forecast_from) ((strings.ago)(Age::of(elapsed))) }
        }
    }
}

/// As an integer, so that -0.4 reads as 0 rather than -0.
fn rounded(value: f64) -> i64 {
    value.round() as i64
}
