use jiff::SignedDuration;

use crate::park_status::{ParkStatus, ReopeningForecast, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    pub(crate) const ALL: [Lang; 2] = [Lang::Es, Lang::En];

    pub(crate) fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag.trim().split('-').next()?;
        Lang::ALL
            .into_iter()
            .find(|lang| primary.eq_ignore_ascii_case(lang.strings().code))
    }

    pub(crate) fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }

    pub(crate) fn strings(self) -> &'static Strings {
        match self {
            Lang::Es => &ES,
            Lang::En => &EN,
        }
    }
}

pub(crate) struct Strings {
    pub(crate) code: &'static str,
    pub(crate) locale: &'static str,
    pub(crate) path: &'static str,
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) question: &'static str,
    pub(crate) headline: fn(Verdict) -> &'static str,
    pub(crate) upstream_failure: &'static str,
    pub(crate) opening_hours: &'static str,
    // The seasons in `schedule::retiro`, which these have to follow.
    pub(crate) seasons: [(&'static str, &'static str); 2],
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
    code: "es",
    locale: "es_ES",
    path: "/es",
    name: "Español",
    title: "¿Está abierto el Retiro hoy? Parque del Retiro, Madrid",
    description: "Consulta si el Parque del Retiro de Madrid está abierto, su horario y si hay cierres o restricciones por alerta meteorológica, con datos del Ayuntamiento actualizados cada minuto.",
    question: "¿Está abierto el Retiro?",
    headline: |verdict| match verdict {
        Verdict::Open => "Abierto",
        Verdict::OpenWithRestrictions => "Abierto con restricciones",
        Verdict::OpenStayNotAdvised => "Abierto, pero no se recomienda permanecer",
        Verdict::ClosureForecast => "Cierre previsto",
        Verdict::Closed => "Cerrado",
        Verdict::Unknown => "Estado desconocido",
    },
    upstream_failure: "No hemos podido consultar el estado",
    opening_hours: "Horario",
    seasons: [
        ("Octubre–Marzo", "6:00–22:00"),
        ("Abril–Septiembre", "6:00–24:00"),
    ],
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
    code: "en",
    locale: "en_GB",
    path: "/en",
    name: "English",
    title: "Is Retiro Park open today? Live status, Madrid",
    description: "Check whether Madrid’s Retiro Park is open, its opening hours and any closures or restrictions from weather alerts, with City Council data refreshed every minute.",
    question: "Is Retiro Park open?",
    headline: |verdict| match verdict {
        Verdict::Open => "Open",
        Verdict::OpenWithRestrictions => "Open with restrictions",
        Verdict::OpenStayNotAdvised => "Open, but staying is not advised",
        Verdict::ClosureForecast => "Closure expected",
        Verdict::Closed => "Closed",
        Verdict::Unknown => "Status unknown",
    },
    upstream_failure: "We couldn’t check the status",
    opening_hours: "Hours",
    seasons: [
        ("October–March", "6:00–22:00"),
        ("April–September", "6:00–midnight"),
    ],
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
