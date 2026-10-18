use std::error::Error;
use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::{MethodRouter, post};
use axum::{Json, Router};
use open_parks::telegram::{
    self, BotToken, Chat, ChatId, ChatMemberStatus, ChatMemberUpdated, Message, RequestError,
    Update, UpdateKind, User,
};
use reqwest::Url;
use serde_json::json;

use crate::support::fake_telegram::{FakeTelegram, Sent, TOKEN};
use crate::support::timeouts::{GENEROUS_TIMEOUT, SHORT_TIMEOUT};

fn client(url: Url, timeout: Duration) -> telegram::Client {
    telegram::Client::new(url, BotToken::new(TOKEN.to_owned()), timeout).unwrap()
}

#[tokio::test]
async fn a_message_is_sent_to_the_chat() {
    let fake = FakeTelegram::start().await;

    client(fake.url.clone(), GENEROUS_TIMEOUT)
        .send_message(ChatId(42), "Retiro is closed")
        .await
        .unwrap();

    assert_eq!(
        fake.sent(),
        [Sent {
            chat_id: 42,
            text: "Retiro is closed".to_owned()
        }]
    );
}

#[tokio::test]
async fn pending_updates_are_received() {
    let fake = FakeTelegram::start().await;
    let id = fake.push_update(
        "message",
        json!({
            "message_id": 1,
            "date": 0,
            "chat": { "id": 42, "type": "private" },
            "from": { "id": 7, "is_bot": false, "first_name": "Ana", "language_code": "es" },
            "text": "/start",
        }),
    );

    let updates = client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(
        updates,
        [Update {
            id,
            kind: UpdateKind::Message(Message {
                chat: Chat { id: ChatId(42) },
                from: Some(User {
                    language_code: Some("es".to_owned())
                }),
                text: Some("/start".to_owned()),
            }),
        }]
    );
}

#[tokio::test]
async fn channel_posts_are_received() {
    let fake = FakeTelegram::start().await;
    fake.push_update(
        "channel_post",
        json!({
            "message_id": 1,
            "date": 0,
            "chat": { "id": -100123, "type": "channel", "title": "Retiro" },
            "text": "/start",
        }),
    );

    let updates = client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await
        .unwrap();

    let kinds: Vec<_> = updates.into_iter().map(|update| update.kind).collect();
    assert_eq!(
        kinds,
        [UpdateKind::ChannelPost(Message {
            chat: Chat {
                id: ChatId(-100123)
            },
            from: None,
            text: Some("/start".to_owned()),
        })]
    );
}

#[tokio::test]
async fn the_bots_membership_changes_are_received() {
    let fake = FakeTelegram::start().await;
    for status in ["member", "left", "kicked"] {
        fake.push_update(
            "my_chat_member",
            json!({
                "chat": { "id": -42, "type": "group", "title": "Runners" },
                "from": { "id": 7, "is_bot": false, "first_name": "Ana" },
                "date": 0,
                "old_chat_member": { "status": "administrator", "user": { "id": 1, "is_bot": true, "first_name": "Bot" } },
                "new_chat_member": { "status": status, "user": { "id": 1, "is_bot": true, "first_name": "Bot" } },
            }),
        );
    }

    let updates = client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await
        .unwrap();

    let kinds: Vec<_> = updates.into_iter().map(|update| update.kind).collect();
    let membership = |status| {
        UpdateKind::MyChatMember(ChatMemberUpdated {
            chat: Chat { id: ChatId(-42) },
            status,
        })
    };
    assert_eq!(
        kinds,
        [
            membership(ChatMemberStatus::Other),
            membership(ChatMemberStatus::Left),
            membership(ChatMemberStatus::Kicked),
        ]
    );
}

#[tokio::test]
async fn updates_of_other_kinds_or_shapes_are_received_as_other() {
    let fake = FakeTelegram::start().await;
    let edited = fake.push_update(
        "edited_message",
        json!({ "message_id": 1, "date": 0, "chat": { "id": 42, "type": "private" }, "text": "/stop" }),
    );
    let chatless = fake.push_update("message", json!({ "message_id": 2, "date": 0 }));
    let start = fake.push_update(
        "message",
        json!({ "message_id": 3, "date": 0, "chat": { "id": 42, "type": "private" }, "text": "/start" }),
    );

    let updates = client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(
        updates,
        [
            Update {
                id: edited,
                kind: UpdateKind::Other
            },
            Update {
                id: chatless,
                kind: UpdateKind::Other
            },
            Update {
                id: start,
                kind: UpdateKind::Message(Message {
                    chat: Chat { id: ChatId(42) },
                    from: None,
                    text: Some("/start".to_owned()),
                }),
            },
        ]
    );
}

fn start_message(chat_id: i64) -> serde_json::Value {
    json!({ "message_id": 1, "date": 0, "chat": { "id": chat_id, "type": "private" }, "text": "/start" })
}

#[tokio::test]
async fn updates_are_asked_for_by_kind_rather_than_relying_on_an_earlier_setting() {
    let fake = FakeTelegram::start().await;

    client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(
        fake.allowed_updates(),
        Some(json!(["message", "channel_post", "my_chat_member"]))
    );
}

#[tokio::test]
async fn an_offset_confirms_the_updates_before_it() {
    let fake = FakeTelegram::start().await;
    let first = fake.push_update("message", start_message(1));
    let second = fake.push_update("message", start_message(2));

    let updates = client(fake.url.clone(), GENEROUS_TIMEOUT)
        .get_updates(Some(first + 1), Duration::ZERO)
        .await
        .unwrap();

    let ids: Vec<_> = updates.iter().map(|update| update.id).collect();
    assert_eq!(ids, [second]);
    assert_eq!(fake.pending_updates().len(), 1);
}

#[tokio::test]
async fn a_long_poll_outlasts_the_request_timeout_and_returns_an_update_once_it_arrives() {
    let fake = FakeTelegram::start().await;
    let telegram = client(fake.url.clone(), SHORT_TIMEOUT);
    let pusher = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(SHORT_TIMEOUT * 3).await;
        pusher.push_update("message", start_message(42));
    });

    let updates = telegram.get_updates(None, GENEROUS_TIMEOUT).await.unwrap();

    assert_eq!(updates.len(), 1);
}

async fn send_after(arrange: impl FnOnce(&FakeTelegram)) -> Result<(), RequestError> {
    let fake = FakeTelegram::start().await;
    arrange(&fake);
    client(fake.url.clone(), GENEROUS_TIMEOUT)
        .send_message(ChatId(42), "Retiro is closed")
        .await
}

#[tokio::test]
async fn a_chat_that_blocked_or_removed_the_bot_is_forbidden() {
    let result = send_after(|fake| fake.block_next_send(42)).await;

    assert!(
        matches!(result, Err(RequestError::Forbidden)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_chat_telegram_doesnt_know_is_not_found() {
    let result = send_after(|fake| fake.lose_chat_on_next_send(42)).await;

    assert!(
        matches!(result, Err(RequestError::ChatNotFound)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_group_upgraded_to_a_supergroup_has_migrated_to_its_new_id() {
    let result = send_after(|fake| fake.migrate_on_next_send(42, -100_042)).await;

    assert!(
        matches!(result, Err(RequestError::Migrated(ChatId(-100_042)))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn too_many_requests_says_how_long_to_wait() {
    let result = send_after(|fake| fake.rate_limit_next_send(42, 7)).await;

    assert!(
        matches!(result, Err(RequestError::RateLimited { retry_after }) if retry_after == Duration::from_secs(7)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn any_other_refusal_is_an_api_error_with_telegrams_description() {
    let result = send_after(|fake| {
        fake.fail_next_send(
            42,
            StatusCode::BAD_REQUEST,
            "Bad Request: message text is empty",
            None,
        )
    })
    .await;

    assert!(
        matches!(
            &result,
            Err(RequestError::Api { code: 400, description }) if description == "Bad Request: message text is empty"
        ),
        "got {result:?}"
    );
}

async fn serving_any_method(handler: MethodRouter) -> Url {
    crate::support::fake_telegram::serving(Router::new().route("/{bot}/{method}", handler)).await
}

fn everything_said_about(error: &RequestError) -> String {
    let mut said = format!("{error:?}");
    let mut source: Option<&dyn Error> = Some(error);
    while let Some(error) = source {
        said += &format!("\n{error}");
        source = error.source();
    }
    said
}

#[tokio::test]
async fn a_telegram_that_never_responds_times_out() {
    let url = serving_any_method(post(std::future::pending::<()>)).await;

    let result = client(url, SHORT_TIMEOUT)
        .send_message(ChatId(42), "Retiro is closed")
        .await;

    let error = result.unwrap_err();
    assert!(matches!(error, RequestError::Timeout), "got {error:?}");
    assert!(!everything_said_about(&error).contains(TOKEN));
}

#[tokio::test]
async fn an_unreachable_telegram_is_a_transport_error_that_keeps_the_token_secret() {
    let unreachable = Url::parse("http://127.0.0.1:0").unwrap();

    let result = client(unreachable, GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await;

    let error = result.unwrap_err();
    assert!(matches!(error, RequestError::Transport(_)), "got {error:?}");
    assert!(!everything_said_about(&error).contains(TOKEN));
}

#[tokio::test]
async fn an_error_page_that_isnt_telegrams_json_is_an_http_status_error() {
    let url = serving_any_method(post(|| async {
        (StatusCode::BAD_GATEWAY, "<html>502 Bad Gateway</html>")
    }))
    .await;

    let result = client(url, GENEROUS_TIMEOUT)
        .send_message(ChatId(42), "Retiro is closed")
        .await;

    assert!(
        matches!(
            result,
            Err(RequestError::HttpStatus(StatusCode::BAD_GATEWAY))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_result_that_doesnt_decode_is_malformed() {
    let url = serving_any_method(post(|| async {
        Json(json!({ "ok": true, "result": { "updates": [] } }))
    }))
    .await;

    let result = client(url, GENEROUS_TIMEOUT)
        .get_updates(None, Duration::ZERO)
        .await;

    assert!(
        matches!(result, Err(RequestError::Malformed(_))),
        "got {result:?}"
    );
}

#[test]
fn the_client_keeps_the_token_out_of_its_debug_output() {
    let telegram = client(Url::parse("http://127.0.0.1").unwrap(), GENEROUS_TIMEOUT);

    assert!(!format!("{telegram:?}").contains(TOKEN));
}

#[test]
fn the_live_client_has_a_valid_url_and_tls_setup() {
    assert!(telegram::Client::live(BotToken::new(TOKEN.to_owned())).is_ok());
}
