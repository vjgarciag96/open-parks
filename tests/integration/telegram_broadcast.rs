use std::time::Duration;

use axum::http::StatusCode;
use open_parks::database::Database;
use open_parks::lang::Lang;
use open_parks::notifier::{Channel, Notification};
use open_parks::park_status::{AlertCode, ParkStatus, ReopeningForecast, Verdict};
use open_parks::public_url::PublicUrl;
use open_parks::telegram::{self, BotToken, Broadcast, ChatId, Pace, Subscriber, Subscribers};
use reqwest::Url;
use tempfile::TempDir;

use crate::support::fake_telegram::{FakeTelegram, Sent, TOKEN};
use crate::support::timeouts::GENEROUS_TIMEOUT;

const FAST: Pace = Pace {
    interval: Duration::ZERO,
    first_retry: Duration::from_millis(10),
    retries: 3,
};

struct Delivery {
    fake: FakeTelegram,
    subscribers: Subscribers,
    broadcast: Broadcast,
    _directory: TempDir,
}

impl Delivery {
    async fn start(subscribers: &[Subscriber]) -> Self {
        Self::paced(subscribers, FAST).await
    }

    async fn paced(subscribed: &[Subscriber], pace: Pace) -> Self {
        let public_url = PublicUrl::new(Url::parse("https://retiro.example").unwrap());
        Self::linking(subscribed, pace, public_url).await
    }

    async fn linking(subscribed: &[Subscriber], pace: Pace, public_url: Option<PublicUrl>) -> Self {
        let directory = TempDir::new().unwrap();
        let database = Database::open(&directory.path().join("open-parks.sqlite3")).unwrap();
        let subscribers = Subscribers::new(database.clone());
        for &subscriber in subscribed {
            subscribers.add(subscriber).await.unwrap();
        }
        let fake = FakeTelegram::start().await;
        let client = telegram::Client::new(
            fake.url.clone(),
            BotToken::new(TOKEN.to_owned()),
            GENEROUS_TIMEOUT,
        )
        .unwrap();
        Self {
            fake,
            broadcast: Broadcast::new(client, Subscribers::new(database), pace, public_url),
            subscribers,
            _directory: directory,
        }
    }

    async fn send(&mut self, notification: Notification) {
        tokio::time::timeout(GENEROUS_TIMEOUT, self.broadcast.send(notification))
            .await
            .expect("the broadcast never finished");
    }

    async fn subscribers(&self) -> Vec<Subscriber> {
        self.subscribers.all().await.unwrap()
    }

    fn sent(&self) -> Vec<Sent> {
        self.fake.sent()
    }

    fn sent_to(&self) -> Vec<i64> {
        self.sent().iter().map(|sent| sent.chat_id).collect()
    }
}

fn subscriber(chat_id: i64, lang: Lang) -> Subscriber {
    Subscriber {
        chat: ChatId(chat_id),
        lang,
    }
}

fn message(chat_id: i64, text: &str) -> Sent {
    Sent {
        chat_id,
        text: text.to_owned(),
    }
}

fn status(code: AlertCode) -> ParkStatus {
    ParkStatus {
        code,
        dates: None,
        hours: None,
        reopening: None,
        notes: None,
    }
}

fn closure() -> Notification {
    Notification {
        old: Verdict::Open,
        new: Verdict::Closed,
        status: ParkStatus {
            dates: Some("13/10/2026".to_owned()),
            reopening: Some(ReopeningForecast::AfterInspections),
            notes: Some("Viento fuerte".to_owned()),
            ..status(AlertCode::Closed)
        },
    }
}

#[tokio::test]
async fn a_closure_reaches_every_subscriber_in_their_language() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::Es), subscriber(2, Lang::En)]).await;

    delivery.send(closure()).await;

    assert_eq!(
        delivery.sent(),
        [
            message(
                1,
                "El Retiro ha cerrado.\n\
                 Cerrado por alerta meteorológica\n\n\
                 Fechas: 13/10/2026\n\
                 Reapertura: Cuando terminen las inspecciones\n\
                 Observaciones: Viento fuerte\n\n\
                 https://retiro.example/es"
            ),
            message(
                2,
                "Retiro has closed.\n\
                 Closed due to a weather alert\n\n\
                 Dates: 13/10/2026\n\
                 Reopening: Once inspections finish\n\
                 Notes: Viento fuerte\n\n\
                 https://retiro.example/en"
            ),
        ]
    );
}

fn change(old: Verdict, code: AlertCode) -> Notification {
    Notification {
        old,
        new: code.verdict(),
        status: status(code),
    }
}

#[tokio::test]
async fn the_wording_tells_a_closure_forecast_an_escalation_and_a_reopening_apart() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::En)]).await;

    for notification in [
        change(Verdict::Open, AlertCode::RedAlertClosure),
        change(Verdict::ClosureForecast, AlertCode::Closed),
        change(Verdict::Closed, AlertCode::RedAlertClosure),
        change(Verdict::ClosureForecast, AlertCode::YellowAlert),
    ] {
        delivery.send(notification).await;
    }

    let link = "\n\nhttps://retiro.example/en";
    assert_eq!(
        delivery.sent(),
        [
            message(
                1,
                &format!("Retiro is expected to close.\nClosure expected{link}")
            ),
            message(
                1,
                &format!("Retiro has closed, as expected.\nClosed due to a weather alert{link}")
            ),
            message(
                1,
                &format!(
                    "Retiro is no longer closed, but is still expected to close.\n\
                     Closure expected{link}"
                )
            ),
            message(
                1,
                &format!("Retiro has reopened.\nOpen with restrictions{link}")
            ),
        ]
    );
}

#[tokio::test]
async fn a_reopening_in_spanish_gives_the_hours() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::Es)]).await;

    let mut reopening = change(Verdict::Closed, AlertCode::Open);
    reopening.status.hours = Some("de 14:00 a 20:00 h".to_owned());

    delivery.send(reopening).await;

    assert_eq!(
        delivery.sent(),
        [message(
            1,
            "El Retiro ha vuelto a abrir.\n\
             Abierto\n\n\
             Horario de la incidencia: de 14:00 a 20:00 h\n\n\
             https://retiro.example/es"
        )]
    );
}

#[tokio::test]
async fn without_a_public_url_a_notice_has_no_link() {
    let mut delivery = Delivery::linking(&[subscriber(1, Lang::En)], FAST, None).await;

    delivery
        .send(change(Verdict::Closed, AlertCode::Open))
        .await;

    assert_eq!(delivery.sent(), [message(1, "Retiro has reopened.\nOpen")]);
}

#[tokio::test]
async fn a_chat_that_blocked_the_bot_or_that_telegram_doesnt_know_is_unsubscribed() {
    let mut delivery = Delivery::start(&[
        subscriber(1, Lang::Es),
        subscriber(2, Lang::En),
        subscriber(3, Lang::Es),
    ])
    .await;
    delivery.fake.block_next_send(1);
    delivery.fake.lose_chat_on_next_send(2);

    delivery.send(closure()).await;

    assert_eq!(delivery.sent_to(), [3]);
    assert_eq!(delivery.subscribers().await, [subscriber(3, Lang::Es)]);
}

#[tokio::test]
async fn a_group_upgraded_to_a_supergroup_is_notified_and_kept_under_its_new_id() {
    let mut delivery = Delivery::start(&[subscriber(-42, Lang::Es), subscriber(1, Lang::En)]).await;
    delivery.fake.migrate_on_next_send(-42, -100_042);

    delivery.send(closure()).await;

    assert_eq!(delivery.sent_to(), [-100_042, 1]);
    assert!(delivery.sent()[0].text.starts_with("El Retiro ha cerrado."));
    assert_eq!(
        delivery.subscribers().await,
        [subscriber(-100_042, Lang::Es), subscriber(1, Lang::En)]
    );
}

fn fail_next_send(fake: &FakeTelegram, chat_id: i64) {
    fake.fail_next_send(
        chat_id,
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        None,
    );
}

#[tokio::test]
async fn a_failed_send_is_retried_without_holding_back_the_other_subscribers() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::Es), subscriber(2, Lang::En)]).await;
    fail_next_send(&delivery.fake, 1);
    fail_next_send(&delivery.fake, 1);

    delivery.send(closure()).await;

    assert_eq!(delivery.sent_to(), [2, 1]);
    assert_eq!(delivery.subscribers().await.len(), 2);
}

#[tokio::test]
async fn a_subscriber_failing_every_retry_misses_that_notification_but_gets_the_next() {
    let subscribers = [subscriber(1, Lang::Es), subscriber(2, Lang::En)];
    let mut delivery = Delivery::start(&subscribers).await;
    let first_attempt_and_every_retry = 1 + FAST.retries;
    for _ in 0..first_attempt_and_every_retry {
        fail_next_send(&delivery.fake, 1);
    }

    delivery.send(closure()).await;
    let closure_sent_to = delivery.sent_to();
    delivery
        .send(change(Verdict::Closed, AlertCode::Open))
        .await;
    let reopening_sent_to = delivery.sent_to()[closure_sent_to.len()..].to_vec();

    assert_eq!(closure_sent_to, [2]);
    assert_eq!(reopening_sent_to, [1, 2]);
    assert_eq!(delivery.subscribers().await, subscribers);
}

#[tokio::test]
async fn a_refusal_that_wont_change_is_not_retried() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::Es)]).await;
    delivery.fake.fail_next_send(
        1,
        StatusCode::BAD_REQUEST,
        "Bad Request: message is too long",
        None,
    );

    delivery.send(closure()).await;
    delivery
        .send(change(Verdict::Closed, AlertCode::Open))
        .await;

    assert_eq!(delivery.sent_to(), [1]);
    assert!(
        delivery.sent()[0]
            .text
            .starts_with("El Retiro ha vuelto a abrir.")
    );
}

#[tokio::test]
async fn sends_are_spaced_by_the_pace_interval() {
    let interval = Duration::from_millis(50);
    let subscribers: Vec<_> = (1..=4)
        .map(|chat_id| subscriber(chat_id, Lang::En))
        .collect();
    let mut delivery = Delivery::paced(&subscribers, Pace { interval, ..FAST }).await;

    delivery.send(closure()).await;
    delivery
        .send(change(Verdict::Closed, AlertCode::Open))
        .await;

    let sent_at = delivery.fake.sent_at();
    assert_eq!(sent_at.len(), 8);
    for gap in sent_at.windows(2).map(|pair| pair[1] - pair[0]) {
        assert!(gap >= interval, "sent {gap:?} apart");
    }
}

#[tokio::test]
async fn too_many_requests_holds_every_send_back_for_as_long_as_telegram_asks() {
    let mut delivery = Delivery::start(&[subscriber(1, Lang::Es), subscriber(2, Lang::En)]).await;
    delivery.fake.rate_limit_next_send(1, 1);
    let started = tokio::time::Instant::now();

    delivery.send(closure()).await;

    assert_eq!(delivery.sent_to(), [2, 1]);
    let first = delivery.fake.sent_at()[0] - started;
    assert!(first >= Duration::from_secs(1), "sent after {first:?}");
}

#[tokio::test]
async fn a_group_upgraded_to_a_supergroup_already_subscribed_is_notified_once() {
    let mut delivery =
        Delivery::start(&[subscriber(-100_042, Lang::En), subscriber(-42, Lang::Es)]).await;
    delivery.fake.migrate_on_next_send(-42, -100_042);

    delivery.send(closure()).await;

    assert_eq!(delivery.sent_to(), [-100_042]);
    assert_eq!(
        delivery.subscribers().await,
        [subscriber(-100_042, Lang::En)]
    );
}
