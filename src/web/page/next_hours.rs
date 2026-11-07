use jiff::{SignedDuration, Timestamp, Zoned};
use maud::{Markup, PreEscaped, html};

use crate::i18n::{Age, Strings};
use crate::open_meteo;
use crate::park::forecast::{Conditions, HourForecast, HourLabel, NextHour};

use super::weather_icon;

/// A forecast past this age still shows, with a note saying how old it is.
const OLD_AFTER: SignedDuration = SignedDuration::from_hours(3);
/// In place of a value Open-Meteo left out, or a weather code it has no icon for.
const MISSING: &str = "–";

pub(super) fn card(strings: &Strings, forecast: &open_meteo::Latest, now: Timestamp) -> Markup {
    let hours = forecast
        .as_ref()
        .map_or(Vec::new(), |fetched| fetched.forecast.next_hours(now));
    html! {
        section.next-hours {
            h2 { (strings.next_hours) }
            @if let (Some(fetched), false) = (forecast, hours.is_empty()) {
                ol.hours tabindex="0" aria-label=(strings.hourly_forecast) {
                    @for item in &hours {
                        @match item {
                            NextHour::Open { hour, label } => @match hour.conditions() {
                                Some(conditions) => (column(strings, hour, *label, &conditions)),
                                None => (blank_column(strings, hour, *label)),
                            },
                            NextHour::Closed { from, until } => (closed_block(strings, from, until)),
                        }
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

fn column(
    strings: &Strings,
    hour: &HourForecast,
    label: HourLabel,
    conditions: &Conditions,
) -> Markup {
    html! {
        li.current[label == HourLabel::Now] {
            (hour_label(strings, hour, label))
            span.icon {
                @if let Some(weather) = conditions.weather {
                    (PreEscaped(weather_icon::svg(weather, conditions.is_day)))
                } @else {
                    (MISSING)
                }
            }
            span.temperature { (rounded(conditions.temperature)) "°" }
            span.rain {
                @if let Some(rain_chance) = conditions.rain_chance {
                    (rain_chance) "%"
                } @else {
                    (MISSING)
                }
            }
            span.gusts {
                @if let Some(gusts) = conditions.gusts {
                    (rounded(gusts))
                } @else {
                    (MISSING)
                }
            }
        }
    }
}

/// The hour's label with its other rows empty, so the columns stay aligned.
fn blank_column(strings: &Strings, hour: &HourForecast, label: HourLabel) -> Markup {
    html! {
        li.current[label == HourLabel::Now] {
            (hour_label(strings, hour, label))
            span.icon {}
            span.temperature {}
            span.rain {}
            span.gusts {}
        }
    }
}

fn hour_label(strings: &Strings, hour: &HourForecast, label: HourLabel) -> Markup {
    html! {
        @if label == HourLabel::DayAndTime {
            span.day { (strings.weekday(hour.at.weekday())) }
        }
        span.hour {
            @if label == HourLabel::Now {
                (strings.hour_now)
            } @else {
                (hour.at.hour()) ":00"
            }
        }
    }
}

fn closed_block(strings: &Strings, from: &Zoned, until: &Zoned) -> Markup {
    html! {
        li.closed { ((strings.closed_block)(from.hour(), until.hour())) }
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
