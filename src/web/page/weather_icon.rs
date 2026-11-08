use crate::park::forecast::Weather;

const CLEAR_SKY: &str = include_str!("../../../assets/weather/clear-sky.svg");
const CLEAR_SKY_NIGHT: &str = include_str!("../../../assets/weather/clear-sky-night.svg");
const MAINLY_CLEAR: &str = include_str!("../../../assets/weather/mainly-clear.svg");
const MAINLY_CLEAR_NIGHT: &str = include_str!("../../../assets/weather/mainly-clear-night.svg");
const PARTLY_CLOUDY: &str = include_str!("../../../assets/weather/partly-cloudy.svg");
const PARTLY_CLOUDY_NIGHT: &str = include_str!("../../../assets/weather/partly-cloudy-night.svg");
const OVERCAST: &str = include_str!("../../../assets/weather/overcast.svg");
const FOG: &str = include_str!("../../../assets/weather/fog.svg");
const RIME_FOG: &str = include_str!("../../../assets/weather/rime-fog.svg");
const LIGHT_DRIZZLE: &str = include_str!("../../../assets/weather/light-drizzle.svg");
const MODERATE_DRIZZLE: &str = include_str!("../../../assets/weather/moderate-drizzle.svg");
const DENSE_DRIZZLE: &str = include_str!("../../../assets/weather/dense-drizzle.svg");
const LIGHT_FREEZING_DRIZZLE: &str =
    include_str!("../../../assets/weather/light-freezing-drizzle.svg");
const DENSE_FREEZING_DRIZZLE: &str =
    include_str!("../../../assets/weather/dense-freezing-drizzle.svg");
const SLIGHT_RAIN: &str = include_str!("../../../assets/weather/slight-rain.svg");
const MODERATE_RAIN: &str = include_str!("../../../assets/weather/moderate-rain.svg");
const HEAVY_RAIN: &str = include_str!("../../../assets/weather/heavy-rain.svg");
const LIGHT_FREEZING_RAIN: &str = include_str!("../../../assets/weather/light-freezing-rain.svg");
const HEAVY_FREEZING_RAIN: &str = include_str!("../../../assets/weather/heavy-freezing-rain.svg");
const SLIGHT_SNOWFALL: &str = include_str!("../../../assets/weather/slight-snowfall.svg");
const MODERATE_SNOWFALL: &str = include_str!("../../../assets/weather/moderate-snowfall.svg");
const HEAVY_SNOWFALL: &str = include_str!("../../../assets/weather/heavy-snowfall.svg");
const SNOW_GRAINS: &str = include_str!("../../../assets/weather/snow-grains.svg");
const SLIGHT_RAIN_SHOWERS: &str = include_str!("../../../assets/weather/slight-rain-showers.svg");
const SLIGHT_RAIN_SHOWERS_NIGHT: &str =
    include_str!("../../../assets/weather/slight-rain-showers-night.svg");
const MODERATE_RAIN_SHOWERS: &str =
    include_str!("../../../assets/weather/moderate-rain-showers.svg");
const MODERATE_RAIN_SHOWERS_NIGHT: &str =
    include_str!("../../../assets/weather/moderate-rain-showers-night.svg");
const VIOLENT_RAIN_SHOWERS: &str = include_str!("../../../assets/weather/violent-rain-showers.svg");
const VIOLENT_RAIN_SHOWERS_NIGHT: &str =
    include_str!("../../../assets/weather/violent-rain-showers-night.svg");
const SLIGHT_SNOW_SHOWERS: &str = include_str!("../../../assets/weather/slight-snow-showers.svg");
const SLIGHT_SNOW_SHOWERS_NIGHT: &str =
    include_str!("../../../assets/weather/slight-snow-showers-night.svg");
const HEAVY_SNOW_SHOWERS: &str = include_str!("../../../assets/weather/heavy-snow-showers.svg");
const HEAVY_SNOW_SHOWERS_NIGHT: &str =
    include_str!("../../../assets/weather/heavy-snow-showers-night.svg");
const THUNDERSTORM: &str = include_str!("../../../assets/weather/thunderstorm.svg");
const THUNDERSTORM_WITH_SLIGHT_HAIL: &str =
    include_str!("../../../assets/weather/thunderstorm-with-slight-hail.svg");
const THUNDERSTORM_WITH_HEAVY_HAIL: &str =
    include_str!("../../../assets/weather/thunderstorm-with-heavy-hail.svg");

/// Weather that shows the sun has a night version with the moon.
pub fn svg(weather: Weather, is_day: bool) -> &'static str {
    match (weather, is_day) {
        (Weather::ClearSky, true) => CLEAR_SKY,
        (Weather::ClearSky, false) => CLEAR_SKY_NIGHT,
        (Weather::MainlyClear, true) => MAINLY_CLEAR,
        (Weather::MainlyClear, false) => MAINLY_CLEAR_NIGHT,
        (Weather::PartlyCloudy, true) => PARTLY_CLOUDY,
        (Weather::PartlyCloudy, false) => PARTLY_CLOUDY_NIGHT,
        (Weather::Overcast, _) => OVERCAST,
        (Weather::Fog, _) => FOG,
        (Weather::RimeFog, _) => RIME_FOG,
        (Weather::LightDrizzle, _) => LIGHT_DRIZZLE,
        (Weather::ModerateDrizzle, _) => MODERATE_DRIZZLE,
        (Weather::DenseDrizzle, _) => DENSE_DRIZZLE,
        (Weather::LightFreezingDrizzle, _) => LIGHT_FREEZING_DRIZZLE,
        (Weather::DenseFreezingDrizzle, _) => DENSE_FREEZING_DRIZZLE,
        (Weather::SlightRain, _) => SLIGHT_RAIN,
        (Weather::ModerateRain, _) => MODERATE_RAIN,
        (Weather::HeavyRain, _) => HEAVY_RAIN,
        (Weather::LightFreezingRain, _) => LIGHT_FREEZING_RAIN,
        (Weather::HeavyFreezingRain, _) => HEAVY_FREEZING_RAIN,
        (Weather::SlightSnowfall, _) => SLIGHT_SNOWFALL,
        (Weather::ModerateSnowfall, _) => MODERATE_SNOWFALL,
        (Weather::HeavySnowfall, _) => HEAVY_SNOWFALL,
        (Weather::SnowGrains, _) => SNOW_GRAINS,
        (Weather::SlightRainShowers, true) => SLIGHT_RAIN_SHOWERS,
        (Weather::SlightRainShowers, false) => SLIGHT_RAIN_SHOWERS_NIGHT,
        (Weather::ModerateRainShowers, true) => MODERATE_RAIN_SHOWERS,
        (Weather::ModerateRainShowers, false) => MODERATE_RAIN_SHOWERS_NIGHT,
        (Weather::ViolentRainShowers, true) => VIOLENT_RAIN_SHOWERS,
        (Weather::ViolentRainShowers, false) => VIOLENT_RAIN_SHOWERS_NIGHT,
        (Weather::SlightSnowShowers, true) => SLIGHT_SNOW_SHOWERS,
        (Weather::SlightSnowShowers, false) => SLIGHT_SNOW_SHOWERS_NIGHT,
        (Weather::HeavySnowShowers, true) => HEAVY_SNOW_SHOWERS,
        (Weather::HeavySnowShowers, false) => HEAVY_SNOW_SHOWERS_NIGHT,
        (Weather::Thunderstorm, _) => THUNDERSTORM,
        (Weather::ThunderstormWithSlightHail, _) => THUNDERSTORM_WITH_SLIGHT_HAIL,
        (Weather::ThunderstormWithHeavyHail, _) => THUNDERSTORM_WITH_HEAVY_HAIL,
    }
}
