use tokio::sync::watch;

use crate::database::{Database, DatabaseError};
use crate::park_status::{AlertCode, ParkStatus, Verdict};
use crate::poller::{Latest, with_sources};

pub async fn start(
    database: Database,
    mut latest: watch::Receiver<Latest>,
) -> Result<(), DatabaseError> {
    let mut last = database.last_code().await?;
    // The status already published counts as a change from the stored one.
    latest.mark_changed();
    tokio::spawn(async move {
        while latest.changed().await.is_ok() {
            let Some(current) = documented_code(&latest.borrow_and_update()) else {
                continue;
            };
            if last == Some(current) {
                continue;
            }
            match database.save_last_code(current).await {
                Ok(()) => last = Some(current),
                Err(error) => eprintln!("storing the last code: {}", with_sources(&error)),
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

fn documented_code(latest: &Latest) -> Option<AlertCode> {
    let code = latest.as_ref().ok()?.status.code;
    documented(code).then_some(code)
}

fn documented(code: AlertCode) -> bool {
    !matches!(code, AlertCode::Unknown(_))
}

fn closure_or_forecast(verdict: Verdict) -> bool {
    matches!(verdict, Verdict::ClosureForecast | Verdict::Closed)
}
