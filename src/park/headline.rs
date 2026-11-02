//! What the page and the bot lead with, decided before it's put into words.

use jiff::civil::Time;

use crate::park::schedule::Schedule;
use crate::park::status::Verdict;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    Open,
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
    pub(crate) fn new(alerts: Alerts, schedule: &Schedule) -> Headline {
        let closed_by_alert = matches!(alerts, Alerts::Checked(Verdict::Closed));
        let closure_ahead =
            matches!(alerts, Alerts::Checked(verdict) if verdict.closure_or_forecast());
        let answer = match (schedule, alerts) {
            (Schedule::Closed { .. }, _) | (_, Alerts::Checked(Verdict::Closed)) => Answer::Closed,
            (Schedule::Open { .. }, Alerts::Unchecked) => Answer::Unchecked,
            (Schedule::Open { .. }, Alerts::Checked(Verdict::ClosureForecast)) => {
                Answer::OpenClosureExpected
            }
            (Schedule::Open { .. }, Alerts::Checked(Verdict::Unknown)) => Answer::Unknown,
            (Schedule::Open { .. }, Alerts::Checked(_)) => Answer::Open,
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
        }
    }
}
