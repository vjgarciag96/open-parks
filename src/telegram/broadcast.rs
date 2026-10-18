use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashSet};
use std::time::Duration;

use reqwest::StatusCode;
use tokio::time::Instant;

use super::{ChatId, Client, RequestError, Subscriber, Subscribers, write_details};
use crate::error::with_sources;
use crate::lang::Lang;
use crate::notifier::{Change, Channel, Notification};
use crate::public_url::PublicUrl;

#[derive(Debug, Clone, Copy)]
pub struct Pace {
    pub interval: Duration,
    /// Each later retry waits twice as long as the one before.
    pub first_retry: Duration,
    pub retries: u32,
}

impl Pace {
    /// Under Telegram's limit of about 30 messages a second, and retrying
    /// for about five minutes.
    pub const LIVE: Pace = Pace {
        interval: Duration::from_millis(40),
        first_retry: Duration::from_secs(5),
        retries: 6,
    };
}

pub struct Broadcast {
    client: Client,
    subscribers: Subscribers,
    pace: Pace,
    /// Where each notice links to, if anywhere.
    public_url: Option<PublicUrl>,
    /// The earliest the next send may go, across notifications.
    next_send: Instant,
}

impl Broadcast {
    pub fn new(
        client: Client,
        subscribers: Subscribers,
        pace: Pace,
        public_url: Option<PublicUrl>,
    ) -> Self {
        Self {
            client,
            subscribers,
            pace,
            public_url,
            next_send: Instant::now(),
        }
    }
}

impl Broadcast {
    /// A chat in `delivered` already has the text.
    async fn deliver(
        &mut self,
        subscriber: &mut Subscriber,
        text: &str,
        delivered: &HashSet<ChatId>,
    ) -> Result<(), RequestError> {
        let mut result = self.send_paced(subscriber.chat, text).await;
        if let Err(RequestError::Migrated(to)) = result {
            self.migrate(subscriber.chat, to).await;
            subscriber.chat = to;
            if delivered.contains(&to) {
                return Ok(());
            }
            result = self.send_paced(to, text).await;
        }
        match result {
            Err(RequestError::Forbidden | RequestError::ChatNotFound) => {
                self.unsubscribe(subscriber.chat).await;
                Ok(())
            }
            result => result,
        }
    }

    async fn send_paced(&mut self, chat: ChatId, text: &str) -> Result<(), RequestError> {
        tokio::time::sleep_until(self.next_send).await;
        let result = self.client.send_message(chat, text).await;
        self.next_send = Instant::now() + self.pace.interval;
        if let Err(RequestError::RateLimited { retry_after }) = result {
            // Telegram limits the bot as a whole, not this chat.
            self.next_send = self.next_send.max(Instant::now() + retry_after);
        }
        result
    }

    async fn migrate(&self, from: ChatId, to: ChatId) {
        if let Err(error) = self.subscribers.migrate(from, to).await {
            eprintln!(
                "moving Telegram chat {} to its new ID {}: {}",
                from.0,
                to.0,
                with_sources(&error)
            );
        }
    }

    async fn unsubscribe(&self, chat: ChatId) {
        if let Err(error) = self.subscribers.remove(chat).await {
            eprintln!(
                "unsubscribing Telegram chat {} that can't be notified: {}",
                chat.0,
                with_sources(&error)
            );
        }
    }

    /// Waiting out a rate limit uses up a retry too, so the retries stay
    /// bounded.
    fn retry_wait(&self, error: &RequestError, retries: u32) -> Option<Duration> {
        if retries >= self.pace.retries || !transient(error) {
            return None;
        }
        Some(match error {
            RequestError::RateLimited { retry_after } => *retry_after,
            _ => self.pace.first_retry * 2u32.saturating_pow(retries),
        })
    }
}

impl Channel for Broadcast {
    async fn send(&mut self, notification: Notification) {
        let subscribers = match self.subscribers.all().await {
            Ok(subscribers) => subscribers,
            Err(error) => {
                eprintln!("listing the Telegram subscribers: {}", with_sources(&error));
                return;
            }
        };
        let now = Instant::now();
        let mut queue: BinaryHeap<_> = subscribers
            .into_iter()
            .enumerate()
            .map(|(order, subscriber)| Pending {
                due: now,
                order,
                subscriber,
                retries: 0,
            })
            .collect();
        let mut order = queue.len();
        // A group that migrated onto a subscribed chat's ID is that chat now.
        let mut delivered = HashSet::new();
        while let Some(mut pending) = queue.pop() {
            tokio::time::sleep_until(pending.due).await;
            if delivered.contains(&pending.subscriber.chat) {
                continue;
            }
            let text = notice(
                &notification,
                pending.subscriber.lang,
                self.public_url.as_ref(),
            );
            let result = self
                .deliver(&mut pending.subscriber, &text, &delivered)
                .await;
            let chat = pending.subscriber.chat;
            let Err(error) = result else {
                delivered.insert(chat);
                continue;
            };
            let Some(wait) = self.retry_wait(&error, pending.retries) else {
                eprintln!(
                    "notifying Telegram chat {}, given up: {}",
                    chat.0,
                    with_sources(&error)
                );
                continue;
            };
            eprintln!(
                "notifying Telegram chat {}, retrying in {wait:?}: {}",
                chat.0,
                with_sources(&error)
            );
            // Last among those due at the same time, so the others go first.
            order += 1;
            queue.push(Pending {
                due: Instant::now() + wait,
                order,
                retries: pending.retries + 1,
                ..pending
            });
        }
    }
}

/// A reply that doesn't decode could mean the message was sent, so it's
/// retried only when its HTTP status says it wasn't.
fn transient(error: &RequestError) -> bool {
    match error {
        RequestError::RateLimited { .. } | RequestError::Timeout | RequestError::Transport(_) => {
            true
        }
        RequestError::HttpStatus(status) => {
            status.is_server_error() || *status == StatusCode::TOO_MANY_REQUESTS
        }
        RequestError::Api { code, .. } => *code == 429 || *code >= 500,
        RequestError::Forbidden
        | RequestError::ChatNotFound
        | RequestError::Migrated(_)
        | RequestError::BodyTooLarge
        | RequestError::Malformed(_) => false,
    }
}

struct Pending {
    due: Instant,
    order: usize,
    subscriber: Subscriber,
    retries: u32,
}

impl Pending {
    fn key(&self) -> Reverse<(Instant, usize)> {
        Reverse((self.due, self.order))
    }
}

impl Ord for Pending {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for Pending {}

fn notice(notification: &Notification, lang: Lang, public_url: Option<&PublicUrl>) -> String {
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
