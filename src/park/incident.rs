//! When an alert applies, read from SIGMA's free-text dates and hours.
//!
//! Only the formats seen live are read: one date as `11/11/2026`, and hours
//! as `12:00-24:00` or `03:00 a 12:00`. Anything else has no window, since a
//! guess could call the park closed when it isn't.

use jiff::civil::{Date, Time};
use jiff::{Timestamp, ToSpan};

use crate::park::schedule::madrid;

/// The span an alert applies to: from `start`, up to but not including `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Window {
    start: Timestamp,
    end: Timestamp,
}

impl Window {
    pub(crate) fn read(dates: &str, hours: &str) -> Option<Window> {
        let day = date(dates.trim())?;
        let (from, to) = hours
            .trim()
            .split_once(" a ")
            .or_else(|| hours.trim().split_once('-'))?;
        let start = at(day, from.trim())?;
        let end = at(day, to.trim())?;
        (start < end).then_some(Window { start, end })
    }

    pub(crate) fn contains(self, now: Timestamp) -> bool {
        self.start <= now && now < self.end
    }
}

fn date(text: &str) -> Option<Date> {
    let mut parts = text.split('/');
    let day = number(parts.next()?, 2)?;
    let month = number(parts.next()?, 2)?;
    let year = number(parts.next()?, 4)?;
    if parts.next().is_some() {
        return None;
    }
    Date::new(year, month as i8, day as i8).ok()
}

/// `HH:MM` on `day` in Madrid, where `24:00` is the midnight that ends it.
fn at(day: Date, text: &str) -> Option<Timestamp> {
    let (hour, minute) = text.split_once(':')?;
    let (hour, minute) = (number(hour, 2)?, number(minute, 2)?);
    let (day, hour) = match (hour, minute) {
        (24, 0) => (day.checked_add(1.day()).ok()?, 0),
        _ => (day, hour),
    };
    let time = Time::new(hour as i8, minute as i8, 0, 0).ok()?;
    let zoned = day.to_datetime(time).to_zoned(madrid()).ok()?;
    Some(zoned.timestamp())
}

fn number(text: &str, digits: usize) -> Option<i16> {
    if text.len() != digits || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
