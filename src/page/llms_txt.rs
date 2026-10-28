use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::api::RETIRO_STATUS_PATH;
use crate::lang::Lang;
use crate::schedule::SEASONS;

use super::{SOURCE_URL, Site};

/// /llms.txt, after https://llmstxt.org: what the site is and where an AI
/// agent finds the answer, in Markdown.
pub(super) fn response(site: &Site) -> Response {
    // Absolute with a public URL, as the links will be read off the site.
    let link = |path: &str| match &site.public_url {
        Some(public_url) => public_url.of(path),
        None => path.to_owned(),
    };
    let strings = Lang::En.strings();
    let hours: String = SEASONS
        .iter()
        .map(|season| {
            format!(
                "- {}–{}: {}\n",
                strings.month(season.first_month),
                strings.month(season.last_month),
                (strings.season_hours)(season)
            )
        })
        .collect();
    let telegram = match &site.telegram_bot_username {
        Some(bot) => format!(
            "- [Telegram bot](https://t.me/{bot}): a message, in English or Spanish, when a weather alert closes the park or is expected to, and when it reopens.\n"
        ),
        None => String::new(),
    };
    let text = format!(
        "# Retiro Abierto

> Whether Madrid's Retiro Park (Parque del Retiro) is open right now: its opening hours, and any closure or restrictions from the City Council's weather alerts, checked every minute.

The park closes every night on a fixed schedule, and the City Council closes or restricts it during weather alerts, such as strong wind or snow. The page combines both into one answer. This site interprets the City Council's data and is not affiliated with it.

Opening hours, Madrid time:

{hours}
## Live status

- [Status page in English]({en}): the answer, any alert with its dates, hours and notes, and when the status was fetched.
- [Status page in Spanish]({es}): the same, in Spanish.
- [Status API]({api}): JSON. `verdict` is what the City Council's alert means for the park: `open`, `open_with_restrictions`, `open_stay_not_advised`, `closure_forecast`, `closed` or `unknown`. It doesn't cover the night closure: `schedule.open_now` says whether the park is within its hours, with `closes_at` or `opens_at`. `fetched_at` is when the alert was last read; if the City Council's service is down it keeps the last one read, so check its age. Until the first read after a restart, the API answers 502 or 504 with `error` and `schedule` only.

## Optional

{telegram}- [Madrid City Council's park closures page]({SOURCE_URL}): the source of the alerts, under CC BY 4.0.
",
        en = link(Lang::En.path()),
        es = link(Lang::Es.path()),
        api = link(RETIRO_STATUS_PATH),
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response()
}
