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
        let (start, open_now) = match schedule::retiro(now) {
            Schedule::Open { .. } => (now, true),
            Schedule::Closed { opens_at } => (opens_at.timestamp(), false),
        };
        let Some(first) = self.hours.iter().position(|hour| hour.contains(start)) else {
            return Vec::new();
        };
        let open_hours: Vec<(&HourForecast, Zoned)> = self.hours[first..]
            .iter()
            .filter_map(|hour| match schedule::retiro(hour.at.timestamp()) {
                Schedule::Open { closes_at } => Some((hour, closes_at)),
                Schedule::Closed { .. } => None,
            })
            .take(NEXT_HOURS)
            .collect();

        let today = now.to_zoned(madrid()).date();
        let first_after_today = open_hours
            .iter()
            .position(|(hour, _)| hour.at.timestamp().to_zoned(madrid()).date() > today);
        let label = |index| {
            if index == 0 && open_now {
                HourLabel::Now
            } else if Some(index) == first_after_today {
                HourLabel::DayAndTime
            } else {
                HourLabel::Time
            }
        };

        let mut items = Vec::new();
        for (index, (hour, _)) in open_hours.iter().enumerate() {
            if index > 0
                && let Some(closed) = closed_between(&open_hours[index - 1].1, hour)
            {
                items.push(closed);
            }
            items.push(NextHour::Open {
                hour,
                label: label(index),
            });
        }
        items
    }
}

/// The closed block before `hour` when the park closed at `closes_at` since
/// the previous open hour. None when it opens again the moment it closes.
fn closed_between<'a>(closes_at: &Zoned, hour: &HourForecast) -> Option<NextHour<'a>> {
    if hour.at.timestamp() < closes_at.timestamp() {
        return None;
    }
    match schedule::retiro(closes_at.timestamp()) {
        Schedule::Closed { opens_at } => Some(NextHour::Closed {
            from: closes_at.clone(),
            until: opens_at,
        }),
        Schedule::Open { .. } => None,
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
