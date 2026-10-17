use jiff::civil::{Date, time};
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

// Main park hours, from https://www.madrid.es/portales/munimadrid/es/Inicio/El-Ayuntamiento/Parques-y-jardines/Patrimonio-Verde/Parques-en-Madrid/Jardines-de-El-Buen-Retiro/?vgnextfmt=default&vgnextoid=499561945d9ec210VgnVCM2000000c205a0aRCRD&vgnextchannel=38bb1914e7d4e210VgnVCM1000000b205a0aRCRD
fn opening(day: Date) -> Zoned {
    madrid_time(day, 6)
}

fn closing(day: Date) -> Zoned {
    if (4..=9).contains(&day.month()) {
        madrid_time(
            day.tomorrow().expect("the park's hours are within range"),
            0,
        )
    } else {
        madrid_time(day, 22)
    }
}

fn madrid_time(day: Date, hour: i8) -> Zoned {
    day.to_datetime(time(hour, 0, 0, 0))
        .to_zoned(madrid())
        .expect("the park's hours are within range")
}

// Embedded at build time: the image has no tz data to read.
fn madrid() -> TimeZone {
    jiff::tz::get!("Europe/Madrid")
}
