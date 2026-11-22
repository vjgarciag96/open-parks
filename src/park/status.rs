use jiff::Timestamp;
use serde::{Serialize, Serializer};

use crate::park::incident::{ClockTime, IncidentHours, Window};
use crate::park::schedule::{self, madrid};

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
        let under_way = || Some(self.window()?.contains(now));
        match verdict {
            Verdict::ClosureForecast if under_way() == Some(true) => Verdict::Closed,
            _ => verdict,
        }
    }

    /// What SIGMA says of a closure at `now`, under way or expected.
    pub(crate) fn alert_closure_at(&self, now: Timestamp) -> Option<AlertClosure> {
        let today = now.to_zoned(madrid()).date();
        // A window on another day says nothing of today's closure, and one
        // that has ended says nothing more.
        let window = self
            .window()
            .filter(|window| window.starts_on(today) && !window.has_ended(now));
        match self.verdict_at(now) {
            Verdict::Closed => {
                let window = window.filter(|window| window.contains(now));
                let end = match window {
                    Some(window) if window.ends_by(schedule::closing(today).timestamp()) => {
                        Some(ClosureEnd::At(window.hours().to))
                    }
                    Some(_) => Some(ClosureEnd::RestOfTheDay),
                    None => self.reopening.map(ClosureEnd::Reopening),
                };
                Some(AlertClosure::UnderWay {
                    window: window.map(Window::hours),
                    end,
                })
            }
            Verdict::ClosureForecast => Some(AlertClosure::Expected {
                window: window.map(Window::hours),
                hours_given: self.hours.is_some(),
            }),
            _ => None,
        }
    }

    fn window(&self) -> Option<Window> {
        Window::read(self.dates.as_deref()?, self.hours.as_deref()?)
    }
}

/// A weather alert's closure, as far as SIGMA says when it starts or ends.
/// The window is there only when it can be read, is today's and hasn't
/// ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlertClosure {
    UnderWay {
        window: Option<IncidentHours>,
        end: Option<ClosureEnd>,
    },
    Expected {
        window: Option<IncidentHours>,
        hours_given: bool,
    },
}

/// When a closure under way ends, as far as SIGMA says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosureEnd {
    /// Its window ends while the park would be open.
    At(ClockTime),
    /// Its window ends after the park's closing time, so the park won't
    /// open again today.
    RestOfTheDay,
    /// SIGMA gives no window for it, only a forecast.
    Reopening(ReopeningForecast),
}

impl AlertClosure {
    pub(crate) fn under_way(self) -> bool {
        matches!(self, AlertClosure::UnderWay { .. })
    }

    pub(crate) fn window(self) -> Option<IncidentHours> {
        match self {
            AlertClosure::UnderWay { window, .. } | AlertClosure::Expected { window, .. } => window,
        }
    }

    /// When a closure under way ends.
    pub(crate) fn end(self) -> Option<ClosureEnd> {
        match self {
            AlertClosure::UnderWay { end, .. } => end,
            AlertClosure::Expected { .. } => None,
        }
    }

    /// Whether a closure is expected without SIGMA saying when.
    pub(crate) fn hours_missing(self) -> bool {
        matches!(
            self,
            AlertClosure::Expected {
                hours_given: false,
                ..
            }
        )
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
