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
