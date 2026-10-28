use jiff::SignedDuration;
use jiff::civil::Time;

use crate::lang::Lang;
use crate::park_status::{ParkStatus, ReopeningForecast, Verdict};
use crate::schedule::Season;

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
    pub(crate) question: &'static str,
    pub(crate) park: &'static str,
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
    pub(crate) ago: fn(Age) -> String,
    pub(crate) stale: &'static str,
    pub(crate) telegram: &'static str,
    pub(crate) source_lead: &'static str,
    pub(crate) source_link: &'static str,
    pub(crate) licence_url: &'static str,
    pub(crate) disclaimer: &'static str,
}

impl Strings {
    pub(crate) fn details<'a>(
        &self,
        status: &'a ParkStatus,
    ) -> impl Iterator<Item = (&'static str, &'a str)> {
        [
            (self.dates, status.dates.as_deref()),
            (self.incident_hours, status.hours.as_deref()),
            (self.reopening, status.reopening.map(self.forecast)),
            (self.notes, status.notes.as_deref()),
        ]
        .into_iter()
        .filter_map(|(term, description)| Some((term, description?)))
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
    question: "¿Está abierto el Retiro?",
    park: "Parque del Retiro",
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
    dates: "Fechas",
    incident_hours: "Horario de la incidencia",
    reopening: "Reapertura",
    forecast: |forecast| match forecast {
        ReopeningForecast::AfterInspections => "Cuando terminen las inspecciones",
        ReopeningForecast::SeeNotes => "Consulta las observaciones",
        ReopeningForecast::Unknown(_) => "Desconocida",
    },
    notes: "Observaciones",
    updated: "Actualizado ",
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
};

const EN: Strings = Strings {
    title: "Is Retiro Park open today? Live status, Madrid",
    description: "Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data refreshed every minute.",
    question: "Is Retiro Park open?",
    park: "Retiro Park",
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
    dates: "Dates",
    incident_hours: "Incident hours",
    reopening: "Reopening",
    forecast: |forecast| match forecast {
        ReopeningForecast::AfterInspections => "Once inspections finish",
        ReopeningForecast::SeeNotes => "See the notes",
        ReopeningForecast::Unknown(_) => "Unknown",
    },
    notes: "Notes",
    updated: "Updated ",
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
};
