use jiff::{SignedDuration, Timestamp, Zoned};

const NEXT_HOURS: usize = 12;
const HOUR: SignedDuration = SignedDuration::from_hours(1);

#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub hours: Vec<HourForecast>,
}

impl Forecast {
    /// Up to 12 consecutive hours, starting with the one that contains `now`.
    /// Empty when no hour contains `now`.
    pub fn next_hours(&self, now: Timestamp) -> &[HourForecast] {
        let Some(first) = self.hours.iter().position(|hour| hour.contains(now)) else {
            return &[];
        };
        let hours = &self.hours[first..];
        &hours[..hours.len().min(NEXT_HOURS)]
    }
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
