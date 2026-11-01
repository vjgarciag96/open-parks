use jiff::Zoned;

#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub hours: Vec<HourForecast>,
}

/// The forecast for the hour from `at` to the next one.
#[derive(Debug, Clone, PartialEq)]
pub struct HourForecast {
    pub at: Zoned,
    /// In °C.
    pub temperature: Option<f64>,
    /// A WMO weather interpretation code.
    pub weather_code: Option<u8>,
    pub is_day: Option<bool>,
    /// In %.
    pub rain_chance: Option<u8>,
    /// In km/h.
    pub gusts: Option<f64>,
}
