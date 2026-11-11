use jiff::Timestamp;
use serde::{Serialize, Serializer};

use crate::park::incident::Window;

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

impl ParkStatus {
    /// The verdict at `now`: a forecast closure is a closure while the
    /// alert's dates and hours say it applies.
    pub fn verdict_at(&self, now: Timestamp) -> Verdict {
        let verdict = self.code.verdict();
        let under_way = || {
            let window = Window::read(self.dates.as_deref()?, self.hours.as_deref()?)?;
            Some(window.contains(now))
        };
        match verdict {
            Verdict::ClosureForecast if under_way() == Some(true) => Verdict::Closed,
            _ => verdict,
        }
    }
}

impl AlertCode {
    pub fn verdict(self) -> Verdict {
        match self {
            AlertCode::Open => Verdict::Open,
            // SIGMA describes code 2 as incidents that affect use of the
            // park, not a closure, so it reads as a restriction.
            AlertCode::Incidents | AlertCode::YellowAlert => Verdict::OpenWithRestrictions,
            AlertCode::OrangeAlert => Verdict::OpenStayNotAdvised,
            // SIGMA labels code 6 as restrictions, but Madrid's protocol
            // closes Retiro under any red alert, and on 11/11/2026 Retiro
            // closed while SIGMA showed code 6.
            AlertCode::RedAlertClosure | AlertCode::RedAlertRestrictions => {
                Verdict::ClosureForecast
            }
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
