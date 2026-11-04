use jiff::SignedDuration;
use jiff::civil::{Date, Time};

use super::Lang;
use crate::park::headline::{Alerts, Answer, ScheduleLine};
use crate::park::schedule::Season;
use crate::park::status::{ParkStatus, ReopeningForecast, Verdict};

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
    pub(crate) opens_at: fn(Time) -> String,
    pub(crate) closes_at: fn(Time) -> String,
    pub(crate) alerts: &'static str,
    pub(crate) alert: fn(Verdict) -> &'static str,
    pub(crate) alerts_unchecked: &'static str,
    pub(crate) next_hours: &'static str,
    pub(crate) hour_now: &'static str,
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
    pub(crate) incident_hours: &'static str,
    pub(crate) reopening: &'static str,
    pub(crate) forecast: fn(ReopeningForecast) -> &'static str,
    pub(crate) notes: &'static str,
    pub(crate) updated: &'static str,
    /// When the status was fetched, in Madrid time, so it still reads right
    /// when quoted later or elsewhere.
    pub(crate) updated_at: fn(Date, Time) -> String,
    pub(crate) ago: fn(Age) -> String,
    pub(crate) stale: &'static str,
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
    ) -> impl Iterator<Item = (&'static str, &'a str)> {
        // SIGMA fills in the dates even with no incident, with what looks
        // like the day of its last bulletin, which reads as an unexplained
        // date under an open park.
        let dates = match status.code.verdict() {
            Verdict::Open => None,
            _ => status.dates.as_deref(),
        };
        [
            (self.dates, dates),
            (self.incident_hours, status.hours.as_deref()),
            (self.reopening, status.reopening.map(self.forecast)),
            (self.notes, status.notes.as_deref()),
        ]
        .into_iter()
        .filter_map(|(term, description)| Some((term, description?)))
    }
}

impl Strings {
    pub(crate) fn answer(&self, answer: Answer) -> &'static str {
        match answer {
            Answer::Open => self.open,
            Answer::OpenClosureExpected => self.open_closure_expected,
            Answer::Unknown => (self.headline)(Verdict::Unknown),
            Answer::Unchecked => self.upstream_failure,
            Answer::Closed => self.closed,
            Answer::ClosedByAlert => (self.headline)(Verdict::Closed),
            Answer::ClosedBySchedule => self.closed_by_schedule,
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
}

fn hours_and_minutes(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// As SIGMA writes its dates, which the page shows too.
fn day_month_year(date: Date) -> String {
    format!("{:02}/{:02}/{}", date.day(), date.month(), date.year())
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
    title: "¿Está abierto el Retiro hoy? Parque del Retiro, Madrid",
    description: "Consulta si el Parque del Retiro de Madrid está abierto, su horario y si hay cierres o restricciones por alerta meteorológica, con datos del Ayuntamiento actualizados cada minuto.",
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
    next_hours: "Próximas horas",
    hour_now: "Ahora",
    hourly_forecast: "Previsión por horas",
    forecast_legend: "Probabilidad de lluvia · Rachas en km/h",
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
    incident_hours: "Horario de la incidencia",
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
    telegram: "Recibe avisos en Telegram",
    source_lead: "Elaborado a partir de ",
    source_link: "datos del Ayuntamiento de Madrid",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.es",
    disclaimer: "Esta web interpreta esos datos y no está vinculada al Ayuntamiento ni avalada por él.",
    forecast_source_lead: "Previsión de ",
};

const EN: Strings = Strings {
    title: "Is Retiro Park open today? Live status, Madrid",
    description: "Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data refreshed every minute.",
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
    next_hours: "Next hours",
    hour_now: "Now",
    hourly_forecast: "Hourly forecast",
    forecast_legend: "Rain chance · Gusts in km/h",
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
    incident_hours: "Incident hours",
    reopening: "Reopening",
    forecast: |forecast| match forecast {
        ReopeningForecast::AfterInspections => "Once inspections finish",
        ReopeningForecast::SeeNotes => "See the notes",
        ReopeningForecast::Unknown(_) => "Unknown",
    },
    notes: "Notes",
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
    telegram: "Get notified on Telegram",
    source_lead: "Based on ",
    source_link: "data from Madrid City Council (Ayuntamiento de Madrid)",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.en",
    disclaimer: "This site interprets that data and is not affiliated with or endorsed by the City Council.",
    forecast_source_lead: "Forecast from ",
};
