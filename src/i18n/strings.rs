use std::borrow::Cow;

use jiff::SignedDuration;
use jiff::civil::{Date, DateTime, Time, Weekday};

use super::Lang;
use crate::park::forecast::{HourLabel, Weather};
use crate::park::headline::{Alerts, Answer, ScheduleLine};
use crate::park::incident::{ClockTime, IncidentDates, IncidentHours};
use crate::park::schedule::Season;
use crate::park::status::{ClosureEnd, ParkStatus, ReopeningForecast, Verdict};

impl Lang {
    pub(crate) fn strings(self) -> &'static Strings {
        match self {
            Lang::Es => &ES,
            Lang::En => &EN,
        }
    }
}

pub(crate) struct Strings {
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) park_name: &'static str,
    pub(crate) question: &'static str,
    pub(crate) headline: fn(Verdict) -> &'static str,
    pub(crate) upstream_failure: &'static str,
    pub(crate) open: &'static str,
    pub(crate) open_closure_expected: &'static str,
    pub(crate) closed: &'static str,
    pub(crate) closed_by_schedule: &'static str,
    /// Under the answer while a weather alert has the park closed.
    pub(crate) closure_cause: &'static str,
    /// When an alert's closure ends, while the park would be open.
    pub(crate) until: fn(ClockTime) -> String,
    /// When an alert's closure ends after the park's closing time.
    pub(crate) rest_of_the_day: &'static str,
    /// When an alert's closure ends, when SIGMA gives no hours.
    pub(crate) reopens: fn(ReopeningForecast) -> Option<&'static str>,
    pub(crate) closure_expected_between: fn(ClockTime, ClockTime) -> String,
    pub(crate) no_closure_hours: &'static str,
    pub(crate) opens_at: fn(Time) -> String,
    pub(crate) closes_at: fn(Time) -> String,
    pub(crate) alerts: &'static str,
    pub(crate) alert: fn(Verdict) -> &'static str,
    pub(crate) alerts_unchecked: &'static str,
    pub(crate) next_hours: &'static str,
    pub(crate) hour_now: &'static str,
    pub(crate) today: &'static str,
    /// Above the forecast, which shows the usual hours.
    pub(crate) alert_closure_note: fn(ClockTime, ClockTime) -> String,
    /// Short names, Monday first.
    pub(crate) weekdays: [&'static str; 7],
    /// Full names as read mid-sentence, Monday first.
    pub(crate) weekdays_full: [&'static str; 7],
    pub(crate) weather: fn(Weather) -> &'static str,
    /// An hour's forecast as one sentence, for screen readers.
    pub(crate) hour_reading: fn(&HourReading) -> String,
    /// The accessible name of the columns that scroll sideways.
    pub(crate) hourly_forecast: &'static str,
    pub(crate) forecast_legend: &'static str,
    pub(crate) forecast_unavailable: &'static str,
    /// Read before `ago`.
    pub(crate) forecast_from: &'static str,
    pub(crate) opening_hours: &'static str,
    /// January first.
    pub(crate) months: [&'static str; 12],
    pub(crate) season_hours: fn(&Season) -> String,
    pub(crate) dates: &'static str,
    /// SIGMA's incident dates, translated where they can be read.
    pub(crate) dates_text: for<'a> fn(&'a str) -> Cow<'a, str>,
    pub(crate) incident_hours: &'static str,
    /// SIGMA's incident hours, translated where they can be read.
    pub(crate) hours_text: for<'a> fn(&'a str) -> Cow<'a, str>,
    pub(crate) reopening: &'static str,
    pub(crate) forecast: fn(ReopeningForecast) -> &'static str,
    pub(crate) notes: &'static str,
    pub(crate) updated: &'static str,
    /// When the status was fetched, in Madrid time, so it still reads right
    /// when quoted later or elsewhere.
    pub(crate) updated_at: fn(Date, Time) -> String,
    pub(crate) ago: fn(Age) -> String,
    pub(crate) stale: &'static str,
    /// Where to check instead, after `stale`.
    pub(crate) stale_link: &'static str,
    /// Above an answer that may be out of date.
    pub(crate) last_known: &'static str,
    /// When the status was last fetched, in Madrid time.
    pub(crate) checked_at: fn(Time) -> String,
    /// As `checked_at`, on an earlier day.
    pub(crate) checked_on: fn(Date, Time) -> String,
    pub(crate) telegram: &'static str,
    pub(crate) source_lead: &'static str,
    pub(crate) source_link: &'static str,
    pub(crate) licence_url: &'static str,
    pub(crate) disclaimer: &'static str,
    pub(crate) forecast_source_lead: &'static str,
}

impl Strings {
    pub(crate) fn details<'a>(
        &self,
        status: &'a ParkStatus,
    ) -> impl Iterator<Item = (&'static str, Cow<'a, str>)> {
        // SIGMA can fill in the dates and hours even with no incident, with
        // what looks like its last bulletin, which reads as an unexplained
        // incident under an open park.
        let (dates, hours) = match status.code.verdict() {
            Verdict::Open => (None, None),
            _ => (status.dates.as_deref(), status.hours.as_deref()),
        };
        [
            (self.dates, dates.map(self.dates_text)),
            (self.incident_hours, hours.map(self.hours_text)),
            (
                self.reopening,
                status
                    .reopening
                    .map(|forecast| Cow::Borrowed((self.forecast)(forecast))),
            ),
            (self.notes, status.notes.as_deref().map(Cow::Borrowed)),
        ]
        .into_iter()
        .filter_map(|(term, description)| Some((term, description?)))
    }
}

impl Strings {
    pub(crate) fn answer(&self, answer: Answer) -> &'static str {
        match answer {
            Answer::Open => self.open,
            Answer::OpenWithRestrictions => (self.headline)(Verdict::OpenWithRestrictions),
            Answer::OpenStayNotAdvised => (self.headline)(Verdict::OpenStayNotAdvised),
            Answer::OpenClosureExpected => self.open_closure_expected,
            Answer::Unknown => (self.headline)(Verdict::Unknown),
            Answer::Unchecked => self.upstream_failure,
            Answer::Closed => self.closed,
            Answer::ClosedByAlert => (self.headline)(Verdict::Closed),
            Answer::ClosedBySchedule => self.closed_by_schedule,
        }
    }

    /// Nothing for a forecast it has no words for.
    pub(crate) fn closure_end(&self, end: ClosureEnd) -> Option<String> {
        match end {
            ClosureEnd::At(time) => Some((self.until)(time)),
            ClosureEnd::RestOfTheDay => Some(self.rest_of_the_day.to_owned()),
            ClosureEnd::Reopening(forecast) => (self.reopens)(forecast).map(str::to_owned),
        }
    }

    pub(crate) fn alert_line(&self, alerts: Alerts) -> &'static str {
        match alerts {
            Alerts::Checked(verdict) => (self.alert)(verdict),
            Alerts::Unchecked => self.alerts_unchecked,
        }
    }

    pub(crate) fn schedule_line(&self, line: ScheduleLine) -> String {
        match line {
            ScheduleLine::ClosesAt(time) => (self.closes_at)(time),
            ScheduleLine::ClosedBySchedule => self.closed_by_schedule.to_owned(),
            ScheduleLine::ClosedUntil(time) => {
                format!("{}. {}", self.closed_by_schedule, (self.opens_at)(time))
            }
        }
    }
}

impl Strings {
    pub(crate) fn month(&self, month: i8) -> &'static str {
        self.months[usize::try_from(month - 1).expect("months start at 1")]
    }

    pub(crate) fn weekday(&self, weekday: Weekday) -> &'static str {
        self.weekdays
            [usize::try_from(weekday.to_monday_zero_offset()).expect("weekdays start at 0")]
    }

    pub(crate) fn weekday_full(&self, weekday: Weekday) -> &'static str {
        self.weekdays_full
            [usize::try_from(weekday.to_monday_zero_offset()).expect("weekdays start at 0")]
    }
}

/// What one hour's column shows, rounded as shown, None where it shows
/// nothing.
pub(crate) struct HourReading {
    pub(crate) label: HourLabel,
    /// In Madrid time.
    pub(crate) at: DateTime,
    pub(crate) weather: Option<Weather>,
    /// In °C.
    pub(crate) temperature: Option<i64>,
    /// In %.
    pub(crate) rain_chance: Option<u8>,
    /// In km/h.
    pub(crate) gusts: Option<i64>,
}

impl HourReading {
    /// The hour worded by `when`, then each value there is, worded by
    /// `value`.
    fn read(
        &self,
        when: fn(HourLabel, DateTime) -> String,
        weather: fn(Weather) -> &'static str,
        value: impl Fn(Value) -> String,
    ) -> String {
        let parts = [
            Some(when(self.label, self.at)),
            self.weather.map(|code| weather(code).to_owned()),
            self.temperature.map(|t| value(Value::Temperature(t))),
            self.rain_chance.map(|p| value(Value::RainChance(p))),
            self.gusts.map(|g| value(Value::Gusts(g))),
        ];
        parts.into_iter().flatten().collect::<Vec<_>>().join(", ")
    }
}

enum Value {
    Temperature(i64),
    RainChance(u8),
    Gusts(i64),
}

/// The hour as its column's label gives it, with its day in full.
fn es_when(label: HourLabel, at: DateTime) -> String {
    let time = format!("{}:00", at.hour());
    match label {
        HourLabel::Now => ES.hour_now.to_owned(),
        HourLabel::TodayAndTime => format!("hoy {time}"),
        HourLabel::DayAndTime => format!("{} {time}", ES.weekday_full(at.weekday())),
        HourLabel::Time => time,
    }
}

/// The hour as its column's label gives it, with its day in full.
fn en_when(label: HourLabel, at: DateTime) -> String {
    let time = format!("{}:00", at.hour());
    match label {
        HourLabel::Now => EN.hour_now.to_owned(),
        HourLabel::TodayAndTime => format!("today {time}"),
        HourLabel::DayAndTime => format!("{} {time}", EN.weekday_full(at.weekday())),
        HourLabel::Time => time,
    }
}

fn hours_and_minutes(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// As SIGMA writes its dates, which the page shows too.
fn day_month_year(date: Date) -> String {
    format!("{:02}/{:02}/{}", date.day(), date.month(), date.year())
}

/// SIGMA's text as it is, for the language it's written in.
fn as_written(text: &str) -> Cow<'_, str> {
    Cow::Borrowed(text)
}

const EN_SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn en_short_month(month: i8) -> &'static str {
    EN_SHORT_MONTHS[usize::try_from(month - 1).expect("months start at 1")]
}

fn en_date(date: Date) -> String {
    format!(
        "{} {} {}",
        date.day(),
        en_short_month(date.month()),
        date.year()
    )
}

/// `time` as `HH:MM` after `lead`, or the language's word for the midnight
/// that ends the day, which takes no lead.
fn clock_time(time: ClockTime, lead: &str, midnight: &str) -> String {
    match time.is_midnight_end() {
        true => midnight.to_owned(),
        false => format!("{lead}{:02}:{:02}", time.hour(), time.minute()),
    }
}

/// `lead` is the article an hour takes where midnight takes none.
fn es_clock_time(time: ClockTime, lead: &str) -> String {
    clock_time(time, lead, "medianoche")
}

fn en_clock_time(time: ClockTime) -> String {
    clock_time(time, "", "midnight")
}

fn en_dates(text: &str) -> Cow<'_, str> {
    match IncidentDates::read(text) {
        Some(IncidentDates::Day(day)) => Cow::Owned(en_date(day)),
        Some(IncidentDates::Range(first, last)) => {
            Cow::Owned(format!("{} – {}", en_date(first), en_date(last)))
        }
        None => Cow::Borrowed(text),
    }
}

fn en_hours(text: &str) -> Cow<'_, str> {
    match IncidentHours::read(text) {
        Some(hours) => Cow::Owned(format!(
            "{} to {}",
            en_clock_time(hours.from),
            en_clock_time(hours.to)
        )),
        None => Cow::Borrowed(text),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Age {
    UnderAMinute,
    Minutes(i64),
    Hours(i64),
    Days(i64),
}

impl Age {
    pub(crate) fn of(elapsed: SignedDuration) -> Age {
        let minutes = elapsed.as_mins();
        if minutes < 1 {
            Age::UnderAMinute
        } else if minutes < 60 {
            Age::Minutes(minutes)
        } else if minutes < 60 * 24 {
            Age::Hours(minutes / 60)
        } else {
            Age::Days(minutes / (60 * 24))
        }
    }
}

const ES: Strings = Strings {
    title: "¿Está abierto hoy el Parque del Retiro de Madrid?",
    description: "Consulta si el Parque del Retiro de Madrid está abierto, su horario y los cierres o restricciones por alerta meteorológica, con datos del Ayuntamiento.",
    park_name: "Parque del Retiro",
    question: "¿Está abierto el Retiro?",
    headline: |verdict| match verdict {
        Verdict::Open => "Abierto",
        Verdict::OpenWithRestrictions => "Abierto con restricciones",
        Verdict::OpenStayNotAdvised => "Abierto, pero no se recomienda permanecer",
        Verdict::ClosureForecast => "Cierre previsto",
        Verdict::Closed => "Cerrado por alerta meteorológica",
        Verdict::Unknown => "Estado desconocido",
    },
    upstream_failure: "No hemos podido consultar el estado",
    open: "Abierto",
    open_closure_expected: "Abierto, cierre previsto por alerta",
    closed: "Cerrado",
    closed_by_schedule: "Cerrado por horario",
    closure_cause: "Por alerta meteorológica",
    until: |time| format!("Hasta {}", es_clock_time(time, "las ")),
    rest_of_the_day: "Durante el resto del día",
    reopens: |forecast| match forecast {
        ReopeningForecast::AfterInspections => Some("Reabrirá cuando terminen las inspecciones"),
        ReopeningForecast::SeeNotes => Some("Reapertura: consulta las observaciones"),
        ReopeningForecast::Unknown(_) => None,
    },
    closure_expected_between: |from, to| {
        format!(
            "Cierre previsto de {} a {}",
            es_clock_time(from, ""),
            es_clock_time(to, "")
        )
    },
    no_closure_hours: "El Ayuntamiento no ha indicado el horario del cierre.",
    opens_at: |time| format!("Abre a las {}", hours_and_minutes(time)),
    closes_at: |time| match time {
        Time::MIN => "Cierra a medianoche".to_owned(),
        time => format!("Cierra a las {}", hours_and_minutes(time)),
    },
    alerts: "Alertas",
    alert: |verdict| match verdict {
        Verdict::Open => "Sin alertas",
        Verdict::OpenWithRestrictions => "Con restricciones",
        Verdict::OpenStayNotAdvised => "No se recomienda permanecer",
        Verdict::ClosureForecast => "Cierre previsto",
        Verdict::Closed => "Cerrado por alerta meteorológica",
        Verdict::Unknown => "Estado desconocido",
    },
    alerts_unchecked: "No hemos podido consultar si hay alertas",
    next_hours: "Previsión durante el horario habitual",
    hour_now: "Ahora",
    today: "Hoy",
    alert_closure_note: |from, to| {
        format!(
            "Cierre por alerta de {} a {}",
            es_clock_time(from, ""),
            es_clock_time(to, "")
        )
    },
    weekdays: ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"],
    weekdays_full: [
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
        "domingo",
    ],
    weather: |weather| match weather {
        Weather::ClearSky => "despejado",
        Weather::MainlyClear => "mayormente despejado",
        Weather::PartlyCloudy => "parcialmente nublado",
        Weather::Overcast => "cubierto",
        Weather::Fog => "niebla",
        Weather::RimeFog => "niebla con escarcha",
        Weather::LightDrizzle => "llovizna ligera",
        Weather::ModerateDrizzle => "llovizna moderada",
        Weather::DenseDrizzle => "llovizna intensa",
        Weather::LightFreezingDrizzle => "llovizna helada ligera",
        Weather::DenseFreezingDrizzle => "llovizna helada intensa",
        Weather::SlightRain => "lluvia ligera",
        Weather::ModerateRain => "lluvia moderada",
        Weather::HeavyRain => "lluvia intensa",
        Weather::LightFreezingRain => "lluvia helada ligera",
        Weather::HeavyFreezingRain => "lluvia helada intensa",
        Weather::SlightSnowfall => "nevada ligera",
        Weather::ModerateSnowfall => "nevada moderada",
        Weather::HeavySnowfall => "nevada intensa",
        Weather::SnowGrains => "granos de nieve",
        Weather::SlightRainShowers => "chubascos ligeros",
        Weather::ModerateRainShowers => "chubascos moderados",
        Weather::ViolentRainShowers => "chubascos violentos",
        Weather::SlightSnowShowers => "chubascos de nieve ligeros",
        Weather::HeavySnowShowers => "chubascos de nieve intensos",
        Weather::Thunderstorm => "tormenta",
        Weather::ThunderstormWithSlightHail => "tormenta con granizo ligero",
        Weather::ThunderstormWithHeavyHail => "tormenta con granizo intenso",
    },
    hour_reading: |reading| {
        reading.read(es_when, ES.weather, |value| match value {
            Value::Temperature(t @ (1 | -1)) => format!("{t} grado"),
            Value::Temperature(t) => format!("{t} grados"),
            Value::RainChance(p) => format!("{p} % de probabilidad de lluvia"),
            Value::Gusts(g) => format!("rachas de {g} km/h"),
        })
    },
    hourly_forecast: "Previsión por horas",
    forecast_legend: "Probabilidad de lluvia. Rachas en km/h",
    forecast_unavailable: "La previsión no está disponible.",
    forecast_from: "Previsión de ",
    opening_hours: "Horario",
    months: [
        "Enero",
        "Febrero",
        "Marzo",
        "Abril",
        "Mayo",
        "Junio",
        "Julio",
        "Agosto",
        "Septiembre",
        "Octubre",
        "Noviembre",
        "Diciembre",
    ],
    season_hours: |season| format!("{}:00–{}:00", season.opens, season.closes),
    dates: "Fechas de la incidencia",
    dates_text: as_written,
    incident_hours: "Horario de la incidencia",
    hours_text: as_written,
    reopening: "Reapertura",
    forecast: |forecast| match forecast {
        ReopeningForecast::AfterInspections => "Cuando terminen las inspecciones",
        ReopeningForecast::SeeNotes => "Consulta las observaciones",
        ReopeningForecast::Unknown(_) => "Desconocida",
    },
    notes: "Observaciones",
    updated: "Actualizado el ",
    updated_at: |date, time| {
        format!(
            "{} a las {} (hora de Madrid)",
            day_month_year(date),
            hours_and_minutes(time)
        )
    },
    ago: |age| match age {
        Age::UnderAMinute => "hace menos de un minuto".to_owned(),
        Age::Minutes(1) => "hace 1 minuto".to_owned(),
        Age::Minutes(minutes) => format!("hace {minutes} minutos"),
        Age::Hours(1) => "hace 1 hora".to_owned(),
        Age::Hours(hours) => format!("hace {hours} horas"),
        Age::Days(1) => "hace 1 día".to_owned(),
        Age::Days(days) => format!("hace {days} días"),
    },
    stale: "Puede que este estado no esté al día.",
    stale_link: "Consulta la web del Ayuntamiento",
    last_known: "Último estado conocido",
    checked_at: |time| format!("Última comprobación a las {}", hours_and_minutes(time)),
    checked_on: |date, time| {
        format!(
            "Última comprobación el {} de {} a las {}",
            date.day(),
            ES.month(date.month()).to_lowercase(),
            hours_and_minutes(time)
        )
    },
    telegram: "Recibe avisos en Telegram",
    source_lead: "Elaborado a partir de ",
    source_link: "datos del Ayuntamiento de Madrid",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.es",
    disclaimer: "Esta web interpreta esos datos y no está vinculada al Ayuntamiento ni avalada por él.",
    forecast_source_lead: "Previsión de ",
};

const EN: Strings = Strings {
    title: "Is Madrid’s Retiro Park open today?",
    description: "Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data.",
    park_name: "Retiro Park",
    question: "Is Retiro Park open?",
    headline: |verdict| match verdict {
        Verdict::Open => "Open",
        Verdict::OpenWithRestrictions => "Open with restrictions",
        Verdict::OpenStayNotAdvised => "Open, but staying is not advised",
        Verdict::ClosureForecast => "Closure expected",
        Verdict::Closed => "Closed due to a weather alert",
        Verdict::Unknown => "Status unknown",
    },
    upstream_failure: "We couldn’t check the status",
    open: "Open",
    open_closure_expected: "Open, closure expected due to an alert",
    closed: "Closed",
    closed_by_schedule: "Closed for the night",
    closure_cause: "Due to a weather alert",
    until: |time| format!("Until {}", en_clock_time(time)),
    rest_of_the_day: "For the rest of the day",
    reopens: |forecast| match forecast {
        ReopeningForecast::AfterInspections => Some("Reopens once inspections finish"),
        ReopeningForecast::SeeNotes => Some("Reopening: see the notes"),
        ReopeningForecast::Unknown(_) => None,
    },
    closure_expected_between: |from, to| {
        format!(
            "Closure expected from {} to {}",
            en_clock_time(from),
            en_clock_time(to)
        )
    },
    no_closure_hours: "The City Council hasn’t given the closure hours.",
    opens_at: |time| format!("Opens at {}", hours_and_minutes(time)),
    closes_at: |time| match time {
        Time::MIN => "Closes at midnight".to_owned(),
        time => format!("Closes at {}", hours_and_minutes(time)),
    },
    alerts: "Alerts",
    alert: |verdict| match verdict {
        Verdict::Open => "No alerts",
        Verdict::OpenWithRestrictions => "Restrictions in place",
        Verdict::OpenStayNotAdvised => "Staying is not advised",
        Verdict::ClosureForecast => "Closure expected",
        Verdict::Closed => "Closed due to a weather alert",
        Verdict::Unknown => "Status unknown",
    },
    alerts_unchecked: "We couldn’t check for weather alerts",
    next_hours: "Forecast during usual opening hours",
    hour_now: "Now",
    today: "Today",
    alert_closure_note: |from, to| {
        format!(
            "Alert closure from {} to {}",
            en_clock_time(from),
            en_clock_time(to)
        )
    },
    weekdays: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    weekdays_full: [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
    weather: |weather| match weather {
        Weather::ClearSky => "clear sky",
        Weather::MainlyClear => "mainly clear",
        Weather::PartlyCloudy => "partly cloudy",
        Weather::Overcast => "overcast",
        Weather::Fog => "fog",
        Weather::RimeFog => "rime fog",
        Weather::LightDrizzle => "light drizzle",
        Weather::ModerateDrizzle => "moderate drizzle",
        Weather::DenseDrizzle => "dense drizzle",
        Weather::LightFreezingDrizzle => "light freezing drizzle",
        Weather::DenseFreezingDrizzle => "dense freezing drizzle",
        Weather::SlightRain => "slight rain",
        Weather::ModerateRain => "moderate rain",
        Weather::HeavyRain => "heavy rain",
        Weather::LightFreezingRain => "light freezing rain",
        Weather::HeavyFreezingRain => "heavy freezing rain",
        Weather::SlightSnowfall => "slight snowfall",
        Weather::ModerateSnowfall => "moderate snowfall",
        Weather::HeavySnowfall => "heavy snowfall",
        Weather::SnowGrains => "snow grains",
        Weather::SlightRainShowers => "slight rain showers",
        Weather::ModerateRainShowers => "moderate rain showers",
        Weather::ViolentRainShowers => "violent rain showers",
        Weather::SlightSnowShowers => "slight snow showers",
        Weather::HeavySnowShowers => "heavy snow showers",
        Weather::Thunderstorm => "thunderstorm",
        Weather::ThunderstormWithSlightHail => "thunderstorm with slight hail",
        Weather::ThunderstormWithHeavyHail => "thunderstorm with heavy hail",
    },
    hour_reading: |reading| {
        reading.read(en_when, EN.weather, |value| match value {
            Value::Temperature(t @ (1 | -1)) => format!("{t} degree"),
            Value::Temperature(t) => format!("{t} degrees"),
            Value::RainChance(p) => format!("{p}% chance of rain"),
            Value::Gusts(g) => format!("gusts of {g} km/h"),
        })
    },
    hourly_forecast: "Hourly forecast",
    forecast_legend: "Rain chance. Gusts in km/h",
    forecast_unavailable: "The forecast isn’t available.",
    forecast_from: "Forecast from ",
    opening_hours: "Hours",
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    season_hours: |season| match season.closes {
        24 => format!("{}:00–midnight", season.opens),
        closes => format!("{}:00–{closes}:00", season.opens),
    },
    dates: "Incident dates",
    dates_text: en_dates,
    incident_hours: "Incident hours",
    hours_text: en_hours,
    reopening: "Reopening",
    forecast: |forecast| match forecast {
        ReopeningForecast::AfterInspections => "Once inspections finish",
        ReopeningForecast::SeeNotes => "See the notes",
        ReopeningForecast::Unknown(_) => "Unknown",
    },
    notes: "Notes (City Council’s text, in Spanish)",
    updated: "Updated ",
    updated_at: |date, time| {
        format!(
            "{} at {} Madrid time",
            day_month_year(date),
            hours_and_minutes(time)
        )
    },
    ago: |age| match age {
        Age::UnderAMinute => "less than a minute ago".to_owned(),
        Age::Minutes(1) => "1 minute ago".to_owned(),
        Age::Minutes(minutes) => format!("{minutes} minutes ago"),
        Age::Hours(1) => "1 hour ago".to_owned(),
        Age::Hours(hours) => format!("{hours} hours ago"),
        Age::Days(1) => "1 day ago".to_owned(),
        Age::Days(days) => format!("{days} days ago"),
    },
    stale: "This status may be out of date.",
    stale_link: "Check the City Council’s page",
    last_known: "Last known status",
    checked_at: |time| format!("Last checked at {}", hours_and_minutes(time)),
    checked_on: |date, time| {
        format!(
            "Last checked on {} {} at {}",
            date.day(),
            en_short_month(date.month()),
            hours_and_minutes(time)
        )
    },
    telegram: "Get notified on Telegram",
    source_lead: "Based on ",
    source_link: "data from Madrid City Council (Ayuntamiento de Madrid)",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.en",
    disclaimer: "This site interprets that data and is not affiliated with or endorsed by the City Council.",
    forecast_source_lead: "Forecast from ",
};
