//! How the park's status reads in a Telegram message.

use std::fmt::Write;

use jiff::Timestamp;

use crate::headline::{Alerts, Headline};
use crate::lang::Lang;
use crate::notifier::{Change, Notification};
use crate::park_status::{ParkStatus, Verdict};
use crate::poller::Latest;
use crate::public_url::PublicUrl;
use crate::schedule::{self, Schedule};
use crate::strings::Strings;

/// The answer to /status and the like: the page's headline, then the
/// details when the status could be fetched.
pub(super) fn status(lang: Lang, latest: &Latest, now: Timestamp) -> String {
    let strings = lang.strings();
    let alerts = match latest {
        Ok(fetched) => Alerts::Checked(fetched.status.code.verdict()),
        Err(_) => Alerts::Unchecked,
    };
    let schedule = schedule::retiro(now);
    let headline = Headline::new(strings, alerts, &schedule);
    let mut text = format!("{}\n{}", strings.question, headline.answer);
    if let Some(schedule) = &headline.schedule {
        write!(text, "\n{schedule}").unwrap();
    }
    // Within hours these answers already say what the alert line would;
    // the page sets them apart, but in a message they'd read twice.
    let answered = matches!(schedule, Schedule::Open { .. })
        && matches!(
            alerts,
            Alerts::Unchecked | Alerts::Checked(Verdict::ClosureForecast | Verdict::Unknown)
        );
    if !answered {
        write!(text, "\n{}: {}", strings.alerts, headline.alert).unwrap();
    }
    if let Ok(fetched) = latest {
        write_details(&mut text, strings, &fetched.status);
    }
    text
}

/// What subscribers are sent when the status changes.
pub(super) fn notice(
    notification: &Notification,
    lang: Lang,
    public_url: Option<&PublicUrl>,
) -> String {
    let strings = lang.strings();
    let status = &notification.status;
    let mut text = format!(
        "{}\n{}",
        (notices(lang).lead)(notification.change()),
        (strings.headline)(notification.new)
    );
    write_details(&mut text, strings, status);
    if let Some(public_url) = public_url {
        text.push_str("\n\n");
        text.push_str(&public_url.of(lang.path()));
    }
    text
}

struct Notices {
    lead: fn(Change) -> &'static str,
}

fn notices(lang: Lang) -> &'static Notices {
    match lang {
        Lang::Es => &ES,
        Lang::En => &EN,
    }
}

const ES: Notices = Notices {
    lead: |change| match change {
        Change::ClosureForecast => "Se prevé que el Retiro cierre.",
        Change::Closure => "El Retiro ha cerrado.",
        Change::Escalation => "El Retiro ha cerrado, como estaba previsto.",
        Change::StillForecast => "El Retiro ya no está cerrado, pero sigue previsto que cierre.",
        Change::Reopening => "El Retiro ha vuelto a abrir.",
    },
};

const EN: Notices = Notices {
    lead: |change| match change {
        Change::ClosureForecast => "Retiro is expected to close.",
        Change::Closure => "Retiro has closed.",
        Change::Escalation => "Retiro has closed, as expected.",
        Change::StillForecast => "Retiro is no longer closed, but is still expected to close.",
        Change::Reopening => "Retiro has reopened.",
    },
};

fn write_details(text: &mut String, strings: &Strings, status: &ParkStatus) {
    for (index, (term, description)) in strings.details(status).enumerate() {
        let separator = if index == 0 { "\n\n" } else { "\n" };
        write!(text, "{separator}{term}: {description}").unwrap();
    }
}
