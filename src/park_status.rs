#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParkStatus {
    pub code: AlertCode,
    pub dates: Option<String>,
    pub hours: Option<String>,
    pub reopening: Option<ReopeningForecast>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Unknown(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReopeningForecast {
    AfterInspections,
    SeeNotes,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Open,
    OpenWithRestrictions,
    /// Open with restrictions, but staying in the park is not advised.
    OpenStayNotAdvised,
    ClosureForecast,
    Closed,
    Unknown,
}
