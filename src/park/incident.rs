//! When an alert applies, read from SIGMA's free-text dates and hours.
//!
//! Only the formats seen live give a window: one date as `11/11/2026`, and
//! hours as `12:00-24:00` or `03:00 a 12:00`. Anything else has no window,
//! since a guess could call the park closed when it isn't. A few more
//! formats can be read for display, where a wrong guess falls back to
//! SIGMA's own text.

use jiff::civil::{Date, Time};
use jiff::{Timestamp, ToSpan};

use crate::park::schedule::madrid;

/// An `HH:MM` as SIGMA writes it, from 00:00 up to 24:00, the midnight that
/// ends the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClockTime {
    hour: i8,
    minute: i8,
}

impl ClockTime {
    fn read(text: &str) -> Option<ClockTime> {
        let (hour, minute) = text.split_once(':')?;
        let (hour, minute) = (number(hour, 2)?, number(minute, 2)?);
        let valid = (hour < 24 && minute < 60) || (hour, minute) == (24, 0);
        valid.then_some(ClockTime {
            hour: hour as i8,
            minute: minute as i8,
        })
    }

    pub(crate) fn hour(self) -> i8 {
        self.hour
    }

    pub(crate) fn minute(self) -> i8 {
        self.minute
    }

    pub(crate) fn is_midnight_end(self) -> bool {
        self.hour == 24
    }

    /// On `day` in Madrid.
    fn on(self, day: Date) -> Option<Timestamp> {
        let (day, hour) = match self.is_midnight_end() {
            true => (day.checked_add(1.day()).ok()?, 0),
            false => (day, self.hour),
        };
        let time = Time::new(hour, self.minute, 0, 0).ok()?;
        let zoned = day.to_datetime(time).to_zoned(madrid()).ok()?;
        Some(zoned.timestamp())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IncidentDates {
    Day(Date),
    /// From the first day through the second, never before it.
    Range(Date, Date),
}

impl IncidentDates {
    /// `11/11/2026`, or `11/10/2026 - 12/10/2026`.
    pub(crate) fn read(text: &str) -> Option<IncidentDates> {
        let text = text.trim();
        match text.split_once(" - ") {
            Some((first, last)) => {
                let (first, last) = (date(first.trim())?, date(last.trim())?);
                (first <= last).then_some(IncidentDates::Range(first, last))
            }
            None => Some(IncidentDates::Day(date(text)?)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IncidentHours {
    pub(crate) from: ClockTime,
    pub(crate) to: ClockTime,
    form: HoursForm,
}

/// How the hours were written, since only the forms seen live give a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoursForm {
    /// `12:00-24:00`
    Dash,
    /// `03:00 a 12:00`
    A,
    /// `de 14:00 a 20:00 h`, seen only in hand-made fixtures.
    DeAH,
}

impl IncidentHours {
    pub(crate) fn read(text: &str) -> Option<IncidentHours> {
        let text = text.trim();
        let de_a_h = text
            .strip_prefix("de ")
            .and_then(|text| text.strip_suffix(" h"));
        let (form, (from, to)) = match de_a_h {
            Some(inner) => (HoursForm::DeAH, inner.split_once(" a ")?),
            None => match text.split_once(" a ") {
                Some(parts) => (HoursForm::A, parts),
                None => (HoursForm::Dash, text.split_once('-')?),
            },
        };
        Some(IncidentHours {
            from: ClockTime::read(from.trim())?,
            to: ClockTime::read(to.trim())?,
            form,
        })
    }
}

/// The span an alert applies to: from `start`, up to but not including `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Window {
    start: Timestamp,
    end: Timestamp,
}

impl Window {
    pub(crate) fn read(dates: &str, hours: &str) -> Option<Window> {
        let IncidentDates::Day(day) = IncidentDates::read(dates)? else {
            return None;
        };
        let hours = IncidentHours::read(hours)?;
        if hours.form == HoursForm::DeAH {
            return None;
        }
        let start = hours.from.on(day)?;
        let end = hours.to.on(day)?;
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

fn number(text: &str, digits: usize) -> Option<i16> {
    if text.len() != digits || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
