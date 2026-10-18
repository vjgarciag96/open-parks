use std::fmt::Write;

use crate::park_status::ParkStatus;
use crate::strings::Strings;

mod bot;
mod broadcast;
mod client;
mod subscribers;
pub use bot::start;
pub use broadcast::{Broadcast, Pace};
pub use client::{
    BotCommand, BotToken, Chat, ChatId, ChatMemberStatus, ChatMemberUpdated, Client, Message,
    RequestError, Update, UpdateKind, User,
};
pub use subscribers::{Subscriber, Subscribers};

fn write_details(text: &mut String, strings: &Strings, status: &ParkStatus) {
    for (index, (term, description)) in strings.details(status).enumerate() {
        let separator = if index == 0 { "\n\n" } else { "\n" };
        write!(text, "{separator}{term}: {description}").unwrap();
    }
}
