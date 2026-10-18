use serde::{Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkStatus {
    pub code: AlertCode,
    pub dates: Option<String>,
    pub hours: Option<String>,
    pub reopening: Option<ReopeningForecast>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCode {
    Open,
    Incidents,
    /// Children's, sports and seniors' areas are closed.
    YellowAlert,
    /// As yellow, plus outdoor events are suspended and staying in the park
    /// is not advised.
    OrangeAlert,
    RedAlertClosure,
    RedAlertRestrictions,
    Closed,
    #[serde(untagged, serialize_with = "unknown")]
    Unknown(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReopeningForecast {
    AfterInspections,
    SeeNotes,
    #[serde(untagged, serialize_with = "unknown")]
    Unknown(i16),
}

impl AlertCode {
    pub fn verdict(self) -> Verdict {
        match self {
            AlertCode::Open => Verdict::Open,
            // SIGMA describes code 2 as incidents that affect use of the
            // park, not a closure, so it reads as a restriction.
            AlertCode::Incidents | AlertCode::YellowAlert | AlertCode::RedAlertRestrictions => {
                Verdict::OpenWithRestrictions
            }
            AlertCode::OrangeAlert => Verdict::OpenStayNotAdvised,
            AlertCode::RedAlertClosure => Verdict::ClosureForecast,
            AlertCode::Closed => Verdict::Closed,
            AlertCode::Unknown(_) => Verdict::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Open,
    OpenWithRestrictions,
    /// Open with restrictions, but staying in the park is not advised.
    OpenStayNotAdvised,
    ClosureForecast,
    Closed,
    Unknown,
}

impl Verdict {
    pub fn closure_or_forecast(self) -> bool {
        matches!(self, Verdict::ClosureForecast | Verdict::Closed)
    }
}

// Clients can't act on a value SIGMA doesn't document, so the raw number
// stays internal.
fn unknown<S: Serializer>(_raw: &i16, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str("unknown")
}
