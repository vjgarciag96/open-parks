use jiff::{SignedDuration, Timestamp, Zoned};

use super::schedule::{self, Schedule, madrid};

const NEXT_HOURS: usize = 12;
const HOUR: SignedDuration = SignedDuration::from_hours(1);

#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub hours: Vec<HourForecast>,
}

impl Forecast {
    /// Up to 12 open hours, from the current hour while the park is open or
    /// from opening time while it's closed. Empty when the data has neither.
    pub fn next_hours(&self, now: Timestamp) -> Vec<NextHour<'_>> {
        let (start, mut label) = match schedule::retiro(now) {
            Schedule::Open { .. } => (now, HourLabel::Now),
            Schedule::Closed { opens_at } => (opens_at.timestamp(), HourLabel::Time),
        };
        let Some(first) = self.hours.iter().position(|hour| hour.contains(start)) else {
            return Vec::new();
        };
        let today = now.to_zoned(madrid()).date();
        let mut day_shown = false;
        let mut last_closes_at: Option<Zoned> = None;
        let mut items = Vec::new();
        let mut open_hours = 0;
        for hour in &self.hours[first..] {
            let Schedule::Open { closes_at } = schedule::retiro(hour.at.timestamp()) else {
                continue;
            };
            let closed_since = last_closes_at
                .replace(closes_at)
                .filter(|from| hour.at.timestamp() >= from.timestamp());
            // No block when the park opens again the moment it closes.
            if let Some(from) = closed_since
                && let Schedule::Closed { opens_at: until } = schedule::retiro(from.timestamp())
            {
                items.push(NextHour::Closed { from, until });
            }
            if !day_shown && hour.at.timestamp().to_zoned(madrid()).date() > today {
                label = HourLabel::DayAndTime;
                day_shown = true;
            }
            items.push(NextHour::Open { hour, label });
            label = HourLabel::Time;
            open_hours += 1;
            if open_hours == NEXT_HOURS {
                break;
            }
        }
        items
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NextHour<'a> {
    Open {
        hour: &'a HourForecast,
        label: HourLabel,
    },
    /// From closing to opening time, between two open hours.
    Closed { from: Zoned, until: Zoned },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HourLabel {
    Now,
    Time,
    DayAndTime,
}

/// The forecast for the hour from `at` to the next one.
#[derive(Debug, Clone, PartialEq)]
pub struct HourForecast {
    pub at: Zoned,
    /// In °C.
    pub temperature: Option<f64>,
    /// A World Meteorological Organization (WMO) weather interpretation code.
    pub weather_code: Option<u8>,
    pub is_day: Option<bool>,
    /// In %.
    pub rain_chance: Option<u8>,
    /// In km/h.
    pub gusts: Option<f64>,
}

impl HourForecast {
    fn contains(&self, now: Timestamp) -> bool {
        let start = self.at.timestamp();
        start <= now && now < start + HOUR
    }
}
