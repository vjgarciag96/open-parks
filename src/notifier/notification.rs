use crate::park::status::{AlertCode, ParkStatus, Verdict};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub old: Verdict,
    pub new: Verdict,
    pub status: ParkStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    ClosureForecast,
    Closure,
    Escalation,
    StillForecast,
    Reopening,
}

impl Notification {
    pub fn change(&self) -> Change {
        match (self.old, self.new) {
            (_, new) if !new.closure_or_forecast() => Change::Reopening,
            (Verdict::ClosureForecast, Verdict::Closed) => Change::Escalation,
            (_, Verdict::Closed) => Change::Closure,
            (Verdict::Closed, _) => Change::StillForecast,
            _ => Change::ClosureForecast,
        }
    }
}

pub fn notification(baseline: Option<AlertCode>, status: &ParkStatus) -> Option<Notification> {
    // An undocumented code says nothing about the park.
    let old = baseline.filter(|&code| documented(code))?.verdict();
    if !documented(status.code) {
        return None;
    }
    let new = status.code.verdict();
    (old != new && (old.closure_or_forecast() || new.closure_or_forecast())).then(|| Notification {
        old,
        new,
        status: status.clone(),
    })
}

pub(super) fn documented(code: AlertCode) -> bool {
    !matches!(code, AlertCode::Unknown(_))
}
