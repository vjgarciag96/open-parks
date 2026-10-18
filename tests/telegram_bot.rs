use std::time::Duration;

use axum::http::StatusCode;
use open_parks::database::Database;
use open_parks::park_status::{AlertCode, ParkStatus, ReopeningForecast};
use open_parks::poller::{Fetched, Latest};
use open_parks::sigma::FetchError;
use open_parks::strings::Lang;
use open_parks::telegram::{self, BotToken, ChatId, Subscriber, Subscribers};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::watch;

mod fake_telegram;
mod timeouts;
use fake_telegram::{FakeTelegram, ProfileCall, Sent, TOKEN};
use timeouts::GENEROUS_TIMEOUT;

const OFTEN: Duration = Duration::from_millis(10);
const USERNAME: &str = "RetiroAbiertoBot";

struct Bot {
    fake: FakeTelegram,
    subscribers: Subscribers,
    latest: watch::Sender<Latest>,
    _directory: TempDir,
}

impl Bot {
    async fn start(latest: Latest) -> Self {
        Self::start_on(FakeTelegram::start().await, latest)
    }

    fn start_on(fake: FakeTelegram, latest: Latest) -> Self {
        let directory = TempDir::new().unwrap();
        let database = Database::open(&directory.path().join("open-parks.sqlite3")).unwrap();
        let client = telegram::Client::new(
            fake.url.clone(),
            BotToken::new(TOKEN.to_owned()),
            GENEROUS_TIMEOUT,
        )
        .unwrap();
        let (latest, receiver) = watch::channel(latest);
        telegram::start(
            client,
            USERNAME.to_owned(),
            Subscribers::new(database.clone()),
            receiver,
        );
        Self {
            fake,
            subscribers: Subscribers::new(database),
            latest,
            _directory: directory,
        }
    }

    async fn handle(&self, kind: &str, payload: Value) {
        let id = self.fake.push_update(kind, payload);
        let handled = async {
            while self
                .fake
                .pending_updates()
                .iter()
                .any(|update| update["update_id"].as_i64() <= Some(id))
            {
                tokio::time::sleep(OFTEN).await;
            }
        };
        tokio::time::timeout(GENEROUS_TIMEOUT, handled)
            .await
            .expect("the bot never confirmed the update");
    }

    async fn subscribers(&self) -> Vec<Subscriber> {
        self.subscribers.all().await.unwrap()
    }

    async fn profile(&self, method: &str) -> Vec<ProfileCall> {
        let calls = || -> Vec<ProfileCall> {
            let calls = self.fake.profile().into_iter();
            calls.filter(|call| call.method == method).collect()
        };
        let set = async {
            while calls().len() < 2 {
                tokio::time::sleep(OFTEN).await;
            }
        };
        tokio::time::timeout(GENEROUS_TIMEOUT, set)
            .await
            .unwrap_or_else(|_| panic!("the bot never called {method} in both languages"));
        calls()
    }

    fn replies(&self) -> Vec<Sent> {
        self.fake.sent()
    }
}

fn closed() -> Latest {
    fetched(ParkStatus {
        code: AlertCode::Closed,
        dates: Some("13/10/2026".to_owned()),
        hours: None,
        reopening: Some(ReopeningForecast::AfterInspections),
        notes: Some("Viento fuerte".to_owned()),
    })
}

fn open() -> Latest {
    fetched(ParkStatus {
        code: AlertCode::Open,
        dates: None,
        hours: None,
        reopening: None,
        notes: None,
    })
}

fn fetched(status: ParkStatus) -> Latest {
    Ok(Fetched {
        status,
        fetched_at: "2026-10-11T09:30:00Z".parse().unwrap(),
    })
}

fn failed() -> Latest {
    Err(FetchError::HttpStatus(StatusCode::SERVICE_UNAVAILABLE))
}

fn private_message(chat_id: i64, language_code: Option<&str>, text: &str) -> Value {
    json!({
        "message_id": 1,
        "date": 0,
        "chat": { "id": chat_id, "type": "private" },
        "from": { "id": 7, "is_bot": false, "first_name": "Ana", "language_code": language_code },
        "text": text,
    })
}

fn call(method: &str, params: Value) -> ProfileCall {
    ProfileCall {
        method: method.to_owned(),
        params,
    }
}

fn subscriber(chat_id: i64, lang: Lang) -> Subscriber {
    Subscriber {
        chat: ChatId(chat_id),
        lang,
    }
}

fn reply(chat_id: i64, text: &str) -> Sent {
    Sent {
        chat_id,
        text: text.to_owned(),
    }
}

#[tokio::test]
async fn start_explains_the_bot_and_replies_with_the_status_without_subscribing() {
    let bot = Bot::start(closed()).await;

    bot.handle("message", private_message(42, Some("en"), "/start"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(
        bot.replies(),
        [reply(
            42,
            "Tells you when Madrid’s Retiro Park closes or reopens, based on City Council \
             data. Send /subscribe to get notified, /status to check now and /unsubscribe \
             to stop.\n\n\
             Is Retiro Park open?\n\
             Closed due to a weather alert\n\n\
             Dates: 13/10/2026\n\
             Reopening: Once inspections finish\n\
             Notes: Viento fuerte"
        )]
    );
}

#[tokio::test]
async fn start_replies_in_the_subscribed_chats_language() {
    let bot = Bot::start(open()).await;
    bot.handle("message", private_message(42, Some("es"), "/subscribe"))
        .await;

    bot.handle("message", private_message(42, Some("en"), "/start"))
        .await;

    assert!(
        bot.replies()[1].text.starts_with("Te avisa cuando"),
        "got {:?}",
        bot.replies()
    );
}

#[tokio::test]
async fn stop_still_unsubscribes() {
    let bot = Bot::start(open()).await;
    bot.handle("message", private_message(42, Some("en"), "/subscribe"))
        .await;

    bot.handle("message", private_message(42, Some("en"), "/stop"))
        .await;

    assert_eq!(bot.subscribers().await, []);
}

#[tokio::test]
async fn subscribe_subscribes_the_chat_and_replies_with_the_status_and_what_is_stored() {
    let bot = Bot::start(closed()).await;

    bot.handle("message", private_message(42, Some("es"), "/subscribe"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(42, Lang::Es)]);
    assert_eq!(
        bot.replies(),
        [reply(
            42,
            "Te avisaremos cuando el Retiro cierre o vuelva a abrir.\n\n\
             ¿Está abierto el Retiro?\n\
             Cerrado por alerta meteorológica\n\n\
             Fechas: 13/10/2026\n\
             Reapertura: Cuando terminen las inspecciones\n\
             Observaciones: Viento fuerte\n\n\
             Para avisarte guardamos el ID de este chat (42) y el idioma (español). \
             /unsubscribe deja de avisarte y los borra."
        )]
    );
}

#[tokio::test]
async fn subscribe_again_keeps_one_subscription_in_the_latest_language() {
    let bot = Bot::start(closed()).await;

    bot.handle("message", private_message(42, Some("es"), "/subscribe"))
        .await;
    bot.handle("message", private_message(42, Some("en"), "/subscribe"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(42, Lang::En)]);
    assert_eq!(bot.replies().len(), 2);
}

#[tokio::test]
async fn subscribe_replies_in_english_to_anyone_not_using_telegram_in_spanish() {
    let bot = Bot::start(open()).await;

    bot.handle("message", private_message(1, Some("es-419"), "/subscribe"))
        .await;
    bot.handle("message", private_message(2, Some("fr"), "/subscribe"))
        .await;
    bot.handle("message", private_message(3, None, "/subscribe"))
        .await;

    assert_eq!(
        bot.subscribers().await,
        [
            subscriber(1, Lang::Es),
            subscriber(2, Lang::En),
            subscriber(3, Lang::En),
        ]
    );
    assert_eq!(
        bot.replies()[2],
        reply(
            3,
            "You’ll be notified when Retiro closes or reopens.\n\n\
             Is Retiro Park open?\n\
             Open\n\n\
             To notify you, we store this chat’s ID (3) and language (English). \
             /unsubscribe ends the notifications and deletes both."
        )
    );
}

#[tokio::test]
async fn subscribe_says_the_status_isnt_known_yet_before_a_fetch_succeeds() {
    let bot = Bot::start(failed()).await;

    bot.handle("message", private_message(42, Some("en"), "/subscribe"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(42, Lang::En)]);
    assert!(
        bot.replies()[0]
            .text
            .contains("We don’t know yet whether Retiro is open."),
        "got {:?}",
        bot.replies()
    );
}

fn group_message(text: &str) -> Value {
    json!({
        "message_id": 1,
        "date": 0,
        "chat": { "id": -42, "type": "group", "title": "Runners" },
        "from": { "id": 7, "is_bot": false, "first_name": "Ana", "language_code": "es" },
        "text": text,
    })
}

#[tokio::test]
async fn a_group_subscribes_with_a_command_addressed_to_the_bot() {
    let bot = Bot::start(open()).await;

    bot.handle("message", group_message("/subscribe@retiroabiertobot"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(-42, Lang::Es)]);
    assert_eq!(bot.replies().len(), 1);
}

#[tokio::test]
async fn commands_addressed_to_another_bot_are_ignored() {
    let bot = Bot::start(open()).await;

    bot.handle("message", group_message("/subscribe@SomeOtherBot"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(bot.replies(), []);
}

#[tokio::test]
async fn messages_that_arent_commands_are_ignored() {
    let bot = Bot::start(open()).await;

    bot.handle(
        "message",
        private_message(42, Some("es"), "hola /subscribe"),
    )
    .await;
    bot.handle("message", private_message(42, Some("es"), "/help"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(bot.replies(), []);
}

#[tokio::test]
async fn a_channel_subscribes_in_english_with_a_post() {
    let bot = Bot::start(open()).await;

    bot.handle(
        "channel_post",
        json!({
            "message_id": 1,
            "date": 0,
            "chat": { "id": -100123, "type": "channel", "title": "Retiro" },
            "text": "/subscribe",
        }),
    )
    .await;

    assert_eq!(bot.subscribers().await, [subscriber(-100123, Lang::En)]);
    assert_eq!(bot.replies()[0].chat_id, -100123);
}

#[tokio::test]
async fn unsubscribe_unsubscribes_and_replies_in_the_chats_language() {
    let bot = Bot::start(open()).await;
    bot.handle("message", private_message(42, Some("es"), "/subscribe"))
        .await;

    // Someone else in the chat, using Telegram in English.
    bot.handle("message", private_message(42, Some("en"), "/unsubscribe"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(
        bot.replies()[1],
        reply(
            42,
            "Ya no te avisaremos. Hemos borrado el ID de este chat y el idioma."
        )
    );
}

#[tokio::test]
async fn unsubscribe_without_a_subscription_still_replies() {
    let bot = Bot::start(open()).await;

    bot.handle("message", private_message(42, Some("en"), "/unsubscribe"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(
        bot.replies(),
        [reply(
            42,
            "You won’t be notified any more. This chat’s ID and language have been deleted."
        )]
    );
}

#[tokio::test]
async fn status_replies_with_the_latest_status_without_subscribing() {
    let bot = Bot::start(open()).await;
    bot.latest.send(closed()).unwrap();

    bot.handle("message", private_message(42, Some("en"), "/status"))
        .await;

    assert_eq!(bot.subscribers().await, []);
    assert_eq!(
        bot.replies(),
        [reply(
            42,
            "Is Retiro Park open?\n\
             Closed due to a weather alert\n\n\
             Dates: 13/10/2026\n\
             Reopening: Once inspections finish\n\
             Notes: Viento fuerte"
        )]
    );
}

#[tokio::test]
async fn status_replies_in_the_subscribed_chats_language() {
    let bot = Bot::start(open()).await;
    bot.handle("message", private_message(42, Some("es"), "/subscribe"))
        .await;

    bot.handle("message", private_message(42, None, "/status"))
        .await;

    assert_eq!(
        bot.replies()[1],
        reply(42, "¿Está abierto el Retiro?\nAbierto")
    );
}

#[tokio::test]
async fn status_says_it_isnt_known_yet_before_a_fetch_succeeds() {
    let bot = Bot::start(failed()).await;

    bot.handle("message", private_message(42, Some("es"), "/status"))
        .await;

    assert_eq!(
        bot.replies(),
        [reply(
            42,
            "Todavía no sabemos si el Retiro está abierto. Prueba de nuevo en unos minutos."
        )]
    );
}

fn membership(chat_id: i64, status: &str) -> Value {
    let bot = json!({ "id": 123456, "is_bot": true, "first_name": "Retiro" });
    json!({
        "chat": { "id": chat_id, "type": "group", "title": "Runners" },
        "from": { "id": 7, "is_bot": false, "first_name": "Ana" },
        "date": 0,
        "old_chat_member": { "status": "member", "user": bot },
        "new_chat_member": { "status": status, "user": bot },
    })
}

#[tokio::test]
async fn removing_the_bot_from_a_chat_unsubscribes_it() {
    let bot = Bot::start(open()).await;
    for chat_id in [-1, -2, -3] {
        bot.handle("message", {
            let mut message = group_message("/subscribe");
            message["chat"]["id"] = chat_id.into();
            message
        })
        .await;
    }

    bot.handle("my_chat_member", membership(-1, "left")).await;
    bot.handle("my_chat_member", membership(-2, "kicked")).await;
    bot.handle("my_chat_member", membership(-3, "administrator"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(-3, Lang::Es)]);
}

#[tokio::test]
async fn the_commands_are_set_at_startup_in_spanish_and_by_default_in_english() {
    let bot = Bot::start(open()).await;

    assert_eq!(
        bot.profile("setMyCommands").await,
        [
            call(
                "setMyCommands",
                json!({
                    "commands": [
                        { "command": "start", "description": "What this bot does" },
                        { "command": "status", "description": "Is Retiro open now?" },
                        { "command": "subscribe", "description": "Get notified when Retiro closes or reopens" },
                        { "command": "unsubscribe", "description": "Stop the notifications" },
                    ],
                }),
            ),
            call(
                "setMyCommands",
                json!({
                    "commands": [
                        { "command": "start", "description": "Qué hace este bot" },
                        { "command": "status", "description": "¿Está abierto el Retiro ahora?" },
                        { "command": "subscribe", "description": "Recibe un aviso cuando el Retiro cierre o vuelva a abrir" },
                        { "command": "unsubscribe", "description": "Deja de recibir avisos" },
                    ],
                    "language_code": "es",
                }),
            ),
        ]
    );
}

#[tokio::test]
async fn the_descriptions_are_set_at_startup_in_spanish_and_by_default_in_english() {
    let bot = Bot::start(open()).await;

    assert_eq!(
        bot.profile("setMyDescription").await,
        [
            call(
                "setMyDescription",
                json!({
                    "description": "Tells you when Madrid’s Retiro Park closes or reopens, \
                        based on City Council data. Send /subscribe to get notified, \
                        /status to check now and /unsubscribe to stop.",
                }),
            ),
            call(
                "setMyDescription",
                json!({
                    "description": "Te avisa cuando el parque del Retiro de Madrid cierra o \
                        vuelve a abrir, a partir de los datos del Ayuntamiento. Envía /subscribe \
                        para recibir avisos, /status para consultarlo ahora y /unsubscribe \
                        para dejar de recibirlos.",
                    "language_code": "es",
                }),
            ),
        ]
    );
    assert_eq!(
        bot.profile("setMyShortDescription").await,
        [
            call(
                "setMyShortDescription",
                json!({ "short_description": "Notifies you when Retiro Park closes or reopens." }),
            ),
            call(
                "setMyShortDescription",
                json!({
                    "short_description": "Te avisa cuando el Retiro cierra o vuelve a abrir.",
                    "language_code": "es",
                }),
            ),
        ]
    );
}

#[tokio::test]
async fn failing_to_set_the_commands_still_sets_the_descriptions_and_answers_commands() {
    let fake = FakeTelegram::start().await;
    fake.fail_profile_method("setMyCommands");
    let bot = Bot::start_on(fake, open());

    bot.profile("setMyDescription").await;
    bot.profile("setMyShortDescription").await;
    bot.handle("message", private_message(42, Some("en"), "/subscribe"))
        .await;

    assert_eq!(bot.subscribers().await, [subscriber(42, Lang::En)]);
    assert_eq!(bot.replies().len(), 1);
}
