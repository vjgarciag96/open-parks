use tokio::sync::watch;

use crate::database::{Database, DatabaseError};
use crate::park_status::AlertCode;
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

fn documented_code(latest: &Latest) -> Option<AlertCode> {
    let code = latest.as_ref().ok()?.status.code;
    (!matches!(code, AlertCode::Unknown(_))).then_some(code)
}
