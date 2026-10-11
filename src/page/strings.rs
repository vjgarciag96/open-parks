use super::Age;
use crate::park_status::{ReopeningForecast, Verdict};

#[derive(Clone, Copy)]
pub(super) enum Lang {
    Es,
    En,
}

impl Lang {
    pub(super) fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag.trim().split('-').next()?;
        [Lang::Es, Lang::En]
            .into_iter()
            .find(|lang| primary.eq_ignore_ascii_case(lang.strings().code))
    }

    pub(super) fn other(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }

    pub(super) fn strings(self) -> &'static Strings {
        match self {
            Lang::Es => &ES,
            Lang::En => &EN,
        }
    }
}

pub(super) struct Strings {
    pub(super) code: &'static str,
    pub(super) path: &'static str,
    pub(super) name: &'static str,
    pub(super) question: &'static str,
    pub(super) headline: fn(Verdict) -> &'static str,
    pub(super) upstream_failure: &'static str,
    pub(super) dates: &'static str,
    pub(super) hours: &'static str,
    pub(super) reopening: &'static str,
    pub(super) forecast: fn(ReopeningForecast) -> &'static str,
    pub(super) notes: &'static str,
    pub(super) updated: &'static str,
    pub(super) ago: fn(Age) -> String,
    pub(super) stale: &'static str,
    pub(super) source_lead: &'static str,
    pub(super) source_link: &'static str,
    pub(super) licence_url: &'static str,
    pub(super) disclaimer: &'static str,
}

const ES: Strings = Strings {
    code: "es",
    path: "/es",
    name: "Español",
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
    dates: "Fechas",
    hours: "Horario",
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
    source_lead: "Elaborado a partir de ",
    source_link: "datos del Ayuntamiento de Madrid",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.es",
    disclaimer: "Esta web interpreta esos datos y no está vinculada al Ayuntamiento ni avalada por él.",
};

const EN: Strings = Strings {
    code: "en",
    path: "/en",
    name: "English",
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
    dates: "Dates",
    hours: "Hours",
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
    source_lead: "Based on ",
    source_link: "data from Madrid City Council (Ayuntamiento de Madrid)",
    licence_url: "https://creativecommons.org/licenses/by/4.0/deed.en",
    disclaimer: "This site interprets that data and is not affiliated with or endorsed by the City Council.",
};
