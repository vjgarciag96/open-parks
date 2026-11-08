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
            } else if index == 0 {
                HourLabel::TodayAndTime
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
    TodayAndTime,
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
    /// None when the temperature or the weather code is missing.
    pub fn conditions(&self) -> Option<Conditions> {
        Some(Conditions {
            weather: Weather::from_wmo(self.weather_code?),
            is_day: self.is_day.unwrap_or(true),
            temperature: self.temperature?,
            rain_chance: self.rain_chance,
            gusts: self.gusts,
        })
    }

    fn contains(&self, now: Timestamp) -> bool {
        let start = self.at.timestamp();
        start <= now && now < start + HOUR
    }
}

/// What one hour's column shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Conditions {
    /// None when Open-Meteo sent a code that isn't a WMO weather code.
    pub weather: Option<Weather>,
    /// True when Open-Meteo leaves it out.
    pub is_day: bool,
    /// In °C.
    pub temperature: f64,
    /// In %.
    pub rain_chance: Option<u8>,
    /// In km/h.
    pub gusts: Option<f64>,
}

/// The weather a WMO code stands for, among the codes Open-Meteo uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weather {
    ClearSky,
    MainlyClear,
    PartlyCloudy,
    Overcast,
    Fog,
    RimeFog,
    LightDrizzle,
    ModerateDrizzle,
    DenseDrizzle,
    LightFreezingDrizzle,
    DenseFreezingDrizzle,
    SlightRain,
    ModerateRain,
    HeavyRain,
    LightFreezingRain,
    HeavyFreezingRain,
    SlightSnowfall,
    ModerateSnowfall,
    HeavySnowfall,
    SnowGrains,
    SlightRainShowers,
    ModerateRainShowers,
    ViolentRainShowers,
    SlightSnowShowers,
    HeavySnowShowers,
    Thunderstorm,
    ThunderstormWithSlightHail,
    ThunderstormWithHeavyHail,
}

impl Weather {
    pub fn from_wmo(code: u8) -> Option<Weather> {
        Some(match code {
            0 => Weather::ClearSky,
            1 => Weather::MainlyClear,
            2 => Weather::PartlyCloudy,
            3 => Weather::Overcast,
            45 => Weather::Fog,
            48 => Weather::RimeFog,
            51 => Weather::LightDrizzle,
            53 => Weather::ModerateDrizzle,
            55 => Weather::DenseDrizzle,
            56 => Weather::LightFreezingDrizzle,
            57 => Weather::DenseFreezingDrizzle,
            61 => Weather::SlightRain,
            63 => Weather::ModerateRain,
            65 => Weather::HeavyRain,
            66 => Weather::LightFreezingRain,
            67 => Weather::HeavyFreezingRain,
            71 => Weather::SlightSnowfall,
            73 => Weather::ModerateSnowfall,
            75 => Weather::HeavySnowfall,
            77 => Weather::SnowGrains,
            80 => Weather::SlightRainShowers,
            81 => Weather::ModerateRainShowers,
            82 => Weather::ViolentRainShowers,
            85 => Weather::SlightSnowShowers,
            86 => Weather::HeavySnowShowers,
            95 => Weather::Thunderstorm,
            96 => Weather::ThunderstormWithSlightHail,
            99 => Weather::ThunderstormWithHeavyHail,
            _ => return None,
        })
    }
}
