use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::database::DatabaseError;
use crate::error::with_sources;
use crate::park_status::{AlertCode, ParkStatus, Verdict};
use crate::poller::Latest;

mod last_status;
pub use last_status::LastStatus;

pub trait Channel: Send + 'static {
    /// The channel's next notification waits until this returns.
    fn send(&mut self, notification: Notification) -> impl Future<Output = ()> + Send;
}

#[derive(Default)]
pub struct Channels {
    queues: Vec<(mpsc::UnboundedSender<Notification>, JoinHandle<()>)>,
}

impl Channels {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, mut channel: impl Channel) -> Self {
        let (queue, mut queued) = mpsc::unbounded_channel();
        let sending = tokio::spawn(async move {
            while let Some(notification) = queued.recv().await {
                channel.send(notification).await;
            }
        });
        self.queues.push((queue, sending));
        self
    }

    fn send(&self, notification: &Notification) {
        for (queue, _) in &self.queues {
            let _ = queue.send(notification.clone());
        }
    }

    /// Waits for each channel to send what it's been given.
    async fn close(self) {
        for (queue, sending) in self.queues {
            drop(queue);
            let _ = sending.await;
        }
    }
}

/// Notifies until the status's sender is dropped, then finishes once every
/// channel has sent what it was given.
pub async fn start(
    last_status: LastStatus,
    mut latest: watch::Receiver<Latest>,
    channels: Channels,
) -> Result<JoinHandle<()>, DatabaseError> {
    let mut baseline = last_status.code().await?;
    let mut stored = baseline;
    // The status already published counts as a change from the stored one.
    latest.mark_changed();
    Ok(tokio::spawn(async move {
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
                match last_status.save(code).await {
                    Ok(()) => stored = baseline,
                    Err(error) => eprintln!("storing the last code: {}", with_sources(&error)),
                }
            }
        }
        channels.close().await;
    }))
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

fn documented_status(latest: &Latest) -> Option<&ParkStatus> {
    let status = &latest.as_ref().ok()?.status;
    documented(status.code).then_some(status)
}

fn documented(code: AlertCode) -> bool {
    !matches!(code, AlertCode::Unknown(_))
}
