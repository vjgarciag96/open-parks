use crate::park_status::Verdict;
use crate::schedule::Schedule;
use crate::strings::Strings;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Alerts {
    Checked(Verdict),
    Unchecked,
}

pub(crate) struct Headline {
    pub(crate) answer: &'static str,
    pub(crate) preview: &'static str,
    pub(crate) alert: &'static str,
    pub(crate) schedule: Option<String>,
}

impl Headline {
    pub(crate) fn new(strings: &Strings, alerts: Alerts, schedule: &Schedule) -> Headline {
        let closed_by_alert = matches!(alerts, Alerts::Checked(Verdict::Closed));
        let closure_ahead =
            matches!(alerts, Alerts::Checked(verdict) if verdict.closure_or_forecast());
        let answer = match (schedule, alerts) {
            (Schedule::Closed { .. }, _) | (_, Alerts::Checked(Verdict::Closed)) => strings.closed,
            (Schedule::Open { .. }, Alerts::Unchecked) => strings.upstream_failure,
            (Schedule::Open { .. }, Alerts::Checked(Verdict::ClosureForecast)) => {
                strings.open_closure_expected
            }
            (Schedule::Open { .. }, Alerts::Checked(Verdict::Unknown)) => {
                (strings.headline)(Verdict::Unknown)
            }
            (Schedule::Open { .. }, Alerts::Checked(_)) => strings.open,
        };
        let preview = match schedule {
            _ if closed_by_alert => (strings.headline)(Verdict::Closed),
            Schedule::Closed { .. } => strings.closed_by_schedule,
            Schedule::Open { .. } => answer,
        };
        let alert = match alerts {
            Alerts::Checked(verdict) => (strings.alert)(verdict),
            Alerts::Unchecked => strings.alerts_unchecked,
        };
        let schedule = match schedule {
            _ if closed_by_alert => None,
            Schedule::Open { .. } if closure_ahead => None,
            Schedule::Open { closes_at } => Some((strings.closes_at)(closes_at.time())),
            Schedule::Closed { .. } if closure_ahead => Some(strings.closed_by_schedule.to_owned()),
            Schedule::Closed { opens_at } => Some(format!(
                "{}. {}",
                strings.closed_by_schedule,
                (strings.opens_at)(opens_at.time())
            )),
        };
        Headline {
            answer,
            preview,
            alert,
            schedule,
        }
    }
}
