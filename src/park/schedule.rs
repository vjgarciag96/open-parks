use jiff::civil::{Date, date, time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

#[derive(Debug)]
pub enum Schedule {
    Open { closes_at: Zoned },
    Closed { opens_at: Zoned },
}

pub fn retiro(now: Timestamp) -> Schedule {
    let now = now.to_zoned(madrid());
    let today = now.date();
    let opens_at = opening(today);
    if now < opens_at {
        return Schedule::Closed { opens_at };
    }
    let closes_at = closing(today);
    if now < closes_at {
        return Schedule::Open { closes_at };
    }
    Schedule::Closed {
        opens_at: opening(today.tomorrow().expect("the park's hours are within range")),
    }
}

/// The season `now` falls in and the one after it, with the days each
/// runs this time round.
pub(crate) fn upcoming_seasons(now: Timestamp) -> [Dated; 2] {
    let current = Dated::containing(now.to_zoned(madrid()).date());
    let next = Dated::containing(
        current
            .last_day
            .tomorrow()
            .expect("the park's hours are within range"),
    );
    [current, next]
}

/// A season with the days, first and last included, it runs one year.
pub(crate) struct Dated {
    pub(crate) season: &'static Season,
    pub(crate) first_day: Date,
    pub(crate) last_day: Date,
}

impl Dated {
    fn containing(day: Date) -> Dated {
        let season = Season::of(day);
        let first_year = if day.month() >= season.first_month {
            day.year()
        } else {
            day.year() - 1
        };
        let last_year = if season.first_month <= season.last_month {
            first_year
        } else {
            first_year + 1
        };
        Dated {
            season,
            first_day: date(first_year, season.first_month, 1),
            last_day: date(last_year, season.last_month, 1).last_of_month(),
        }
    }
}

// Main park hours, from https://www.madrid.es/portales/munimadrid/es/Inicio/El-Ayuntamiento/Parques-y-jardines/Patrimonio-Verde/Parques-en-Madrid/Jardines-de-El-Buen-Retiro/?vgnextfmt=default&vgnextoid=499561945d9ec210VgnVCM2000000c205a0aRCRD&vgnextchannel=38bb1914e7d4e210VgnVCM1000000b205a0aRCRD
pub(crate) const SEASONS: [Season; 2] = [
    Season {
        first_month: 10,
        last_month: 3,
        opens: 6,
        closes: 22,
    },
    Season {
        first_month: 4,
        last_month: 9,
        opens: 6,
        closes: 24,
    },
];

/// The hours for the months from `first_month` to `last_month`, which may
/// run across the new year.
pub(crate) struct Season {
    pub(crate) first_month: i8,
    pub(crate) last_month: i8,
    pub(crate) opens: i8,
    /// 24 is midnight, at the end of the day.
    pub(crate) closes: i8,
}

impl Season {
    fn of(day: Date) -> &'static Season {
        SEASONS
            .iter()
            .find(|season| season.contains(day.month()))
            .expect("the seasons cover every month")
    }

    fn contains(&self, month: i8) -> bool {
        if self.first_month <= self.last_month {
            (self.first_month..=self.last_month).contains(&month)
        } else {
            month >= self.first_month || month <= self.last_month
        }
    }
}

fn opening(day: Date) -> Zoned {
    madrid_time(day, Season::of(day).opens)
}

fn closing(day: Date) -> Zoned {
    match Season::of(day).closes {
        24 => madrid_time(
            day.tomorrow().expect("the park's hours are within range"),
            0,
        ),
        closes => madrid_time(day, closes),
    }
}

fn madrid_time(day: Date, hour: i8) -> Zoned {
    day.to_datetime(time(hour, 0, 0, 0))
        .to_zoned(madrid())
        .expect("the park's hours are within range")
}

// Embedded at build time: the image has no tz data to read.
pub(crate) fn madrid() -> TimeZone {
    jiff::tz::get!("Europe/Madrid")
}
