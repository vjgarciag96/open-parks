//! What the page and the bot lead with, decided before it's put into words.

use jiff::Timestamp;
use jiff::civil::Time;

use crate::park::incident::IncidentHours;
use crate::park::schedule::Schedule;
use crate::park::status::{AlertClosure, ParkStatus, Verdict};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Alerts {
    Checked(Verdict),
    Unchecked,
}

pub(crate) struct Headline {
    pub(crate) answer: Answer,
    /// The answer as a link preview gives it, on its own, so a closure says
    /// what closed the park.
    pub(crate) preview: Answer,
    pub(crate) alerts: Alerts,
    pub(crate) schedule: Option<ScheduleLine>,
    pub(crate) closure: Option<AlertClosure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    Open,
    OpenWithRestrictions,
    OpenStayNotAdvised,
    OpenClosureExpected,
    /// The alerts were checked, but SIGMA's code isn't one it documents.
    Unknown,
    /// The park's hours have it open, but the alerts couldn't be checked.
    Unchecked,
    Closed,
    ClosedByAlert,
    ClosedBySchedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleLine {
    ClosesAt(Time),
    ClosedBySchedule,
    /// Closed by the schedule, and when it opens.
    ClosedUntil(Time),
}

impl Headline {
    /// `status` is None when SIGMA couldn't be read.
    pub(crate) fn at(status: Option<&ParkStatus>, schedule: &Schedule, now: Timestamp) -> Headline {
        let alerts = match status {
            Some(status) => Alerts::Checked(status.verdict_at(now)),
            None => Alerts::Unchecked,
        };
        let closed_by_alert = matches!(alerts, Alerts::Checked(Verdict::Closed));
        let closure_ahead =
            matches!(alerts, Alerts::Checked(verdict) if verdict.closure_or_forecast());
        let answer = match (schedule, alerts) {
            (Schedule::Closed { .. }, _) | (_, Alerts::Checked(Verdict::Closed)) => Answer::Closed,
            (Schedule::Open { .. }, Alerts::Unchecked) => Answer::Unchecked,
            (Schedule::Open { .. }, Alerts::Checked(Verdict::OpenWithRestrictions)) => {
                Answer::OpenWithRestrictions
            }
            (Schedule::Open { .. }, Alerts::Checked(Verdict::OpenStayNotAdvised)) => {
                Answer::OpenStayNotAdvised
            }
            (Schedule::Open { .. }, Alerts::Checked(Verdict::ClosureForecast)) => {
                Answer::OpenClosureExpected
            }
            (Schedule::Open { .. }, Alerts::Checked(Verdict::Unknown)) => Answer::Unknown,
            (Schedule::Open { .. }, Alerts::Checked(Verdict::Open)) => Answer::Open,
        };
        let preview = match schedule {
            _ if closed_by_alert => Answer::ClosedByAlert,
            Schedule::Closed { .. } => Answer::ClosedBySchedule,
            Schedule::Open { .. } => answer,
        };
        let schedule = match schedule {
            _ if closed_by_alert => None,
            Schedule::Open { .. } if closure_ahead => None,
            Schedule::Open { closes_at } => Some(ScheduleLine::ClosesAt(closes_at.time())),
            Schedule::Closed { .. } if closure_ahead => Some(ScheduleLine::ClosedBySchedule),
            Schedule::Closed { opens_at } => Some(ScheduleLine::ClosedUntil(opens_at.time())),
        };
        Headline {
            answer,
            preview,
            alerts,
            schedule,
            closure: status.and_then(|status| status.alert_closure_at(now)),
        }
    }

    /// The window of an expected closure, worth a line under the answer only
    /// while the schedule has the park open: closed for the night, the answer
    /// already says it's closed.
    pub(crate) fn expected_closure_window(&self) -> Option<IncidentHours> {
        match self.answer {
            Answer::OpenClosureExpected => self.closure.and_then(AlertClosure::window),
            _ => None,
        }
    }
}
