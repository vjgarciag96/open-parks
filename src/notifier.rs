use tokio::sync::{mpsc, watch};

use crate::database::{Database, DatabaseError};
use crate::park_status::{AlertCode, ParkStatus, Verdict};
use crate::poller::{Latest, with_sources};

pub trait Channel: Send + 'static {
    /// The channel's next notification waits until this returns.
    fn send(&mut self, notification: Notification) -> impl Future<Output = ()> + Send;
}

#[derive(Default)]
pub struct Channels {
    queues: Vec<mpsc::UnboundedSender<Notification>>,
}

impl Channels {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, mut channel: impl Channel) -> Self {
        let (queue, mut queued) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(notification) = queued.recv().await {
                channel.send(notification).await;
            }
        });
        self.queues.push(queue);
        self
    }

    fn send(&self, notification: &Notification) {
        for queue in &self.queues {
            let _ = queue.send(notification.clone());
        }
    }
}

pub async fn start(
    database: Database,
    mut latest: watch::Receiver<Latest>,
    channels: Channels,
) -> Result<(), DatabaseError> {
    let mut baseline = database.last_code().await?;
    let mut stored = baseline;
    // The status already published counts as a change from the stored one.
    latest.mark_changed();
    tokio::spawn(async move {
        while latest.changed().await.is_ok() {
            let (code, notification) = {
                let latest = latest.borrow_and_update();
                let Some(status) = documented_status(&latest) else {
                    continue;
                };
                (status.code, notification(baseline, status))
            };
            // Sent before it's stored, so a crash in between sends it again
            // after the restart rather than never.
            if let Some(notification) = notification {
                channels.send(&notification);
            }
            baseline = Some(code);
            if stored != baseline {
                match database.save_last_code(code).await {
                    Ok(()) => stored = baseline,
                    Err(error) => eprintln!("storing the last code: {}", with_sources(&error)),
                }
            }
        }
    });
    Ok(())
}

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
            (_, new) if !closure_or_forecast(new) => Change::Reopening,
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
    (old != new && (closure_or_forecast(old) || closure_or_forecast(new))).then(|| Notification {
        old,
        new,
        status: status.clone(),
    })
}

fn documented_status(latest: &Latest) -> Option<&ParkStatus> {
    let status = &latest.as_ref().ok()?.status;
    documented(status.code).then_some(status)
}

fn documented(code: AlertCode) -> bool {
    !matches!(code, AlertCode::Unknown(_))
}

fn closure_or_forecast(verdict: Verdict) -> bool {
    matches!(verdict, Verdict::ClosureForecast | Verdict::Closed)
}
