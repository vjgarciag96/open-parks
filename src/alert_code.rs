/// A park's status as reported by Madrid SIGMA's `ALERTA_DESCRIPCION` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCode {
    /// 1, "Abierto según horario habitual": open, normal hours.
    Open,
    /// 2, "Hay incidencias que afectan a la utilización del parque":
    /// incidents affecting use of the park.
    Incidents,
    /// 3, "Previsión alerta amarilla": kids, sports and seniors areas closed.
    YellowAlert,
    /// 4, "Previsión alerta naranja": as yellow, plus outdoor events
    /// suspended; staying in the park not recommended.
    OrangeAlert,
    /// 5, "Previsión alerta roja": park closed during the period.
    RedAlertClosure,
    /// 6, "Previsión alerta roja": zone and event restrictions per hours,
    /// not a closure.
    RedAlertRestrictions,
    /// 7, "Cerrado": closed.
    Closed,
    /// A code SIGMA doesn't document (yet), kept as received.
    Unknown(i16),
}

impl From<i16> for AlertCode {
    fn from(raw: i16) -> Self {
        match raw {
            1 => AlertCode::Open,
            2 => AlertCode::Incidents,
            3 => AlertCode::YellowAlert,
            4 => AlertCode::OrangeAlert,
            5 => AlertCode::RedAlertClosure,
            6 => AlertCode::RedAlertRestrictions,
            7 => AlertCode::Closed,
            other => AlertCode::Unknown(other),
        }
    }
}
