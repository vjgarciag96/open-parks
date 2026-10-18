use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio::time::Instant;

pub const TOKEN: &str = "123456:fake-token";

#[derive(Clone)]
pub struct FakeTelegram {
    pub url: Url,
    state: Arc<Mutex<FakeState>>,
    pushed: Arc<Notify>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub chat_id: i64,
    pub text: String,
}

#[derive(Default)]
struct FakeState {
    next_update_id: i64,
    updates: Vec<Value>,
    sent: Vec<Sent>,
    sent_at: Vec<Instant>,
    allowed_updates: Option<Value>,
    failures: HashMap<i64, VecDeque<(StatusCode, Value)>>,
    profile: Vec<ProfileCall>,
    failing_profile_methods: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileCall {
    pub method: String,
    pub params: Value,
}

impl FakeTelegram {
    pub async fn start() -> Self {
        let fake = Self {
            url: Url::parse("http://127.0.0.1").unwrap(),
            state: Arc::default(),
            pushed: Arc::default(),
        };
        let router = Router::new()
            .route("/{bot}/{method}", post(handle))
            .with_state(fake.clone());
        let url = serving(router).await;
        Self { url, ..fake }
    }

    pub fn push_update(&self, kind: &str, payload: Value) -> i64 {
        let mut state = self.state.lock().unwrap();
        state.next_update_id += 1;
        let id = state.next_update_id;
        state
            .updates
            .push(json!({ "update_id": id, kind: payload }));
        drop(state);
        self.pushed.notify_waiters();
        id
    }

    pub fn pending_updates(&self) -> Vec<Value> {
        self.state.lock().unwrap().updates.clone()
    }

    pub fn allowed_updates(&self) -> Option<Value> {
        self.state.lock().unwrap().allowed_updates.clone()
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.state.lock().unwrap().sent.clone()
    }

    pub fn sent_at(&self) -> Vec<Instant> {
        self.state.lock().unwrap().sent_at.clone()
    }

    pub fn profile(&self) -> Vec<ProfileCall> {
        self.state.lock().unwrap().profile.clone()
    }

    pub fn fail_profile_method(&self, method: &str) {
        self.state
            .lock()
            .unwrap()
            .failing_profile_methods
            .insert(method.to_owned());
    }

    pub fn fail_next_send(
        &self,
        chat_id: i64,
        status: StatusCode,
        description: &str,
        parameters: Option<Value>,
    ) {
        let mut body = json!({
            "ok": false,
            "error_code": status.as_u16(),
            "description": description,
        });
        if let Some(parameters) = parameters {
            body["parameters"] = parameters;
        }
        self.state
            .lock()
            .unwrap()
            .failures
            .entry(chat_id)
            .or_default()
            .push_back((status, body));
    }

    pub fn block_next_send(&self, chat_id: i64) {
        self.fail_next_send(
            chat_id,
            StatusCode::FORBIDDEN,
            "Forbidden: bot was blocked by the user",
            None,
        );
    }

    pub fn lose_chat_on_next_send(&self, chat_id: i64) {
        self.fail_next_send(
            chat_id,
            StatusCode::BAD_REQUEST,
            "Bad Request: chat not found",
            None,
        );
    }

    pub fn migrate_on_next_send(&self, chat_id: i64, to: i64) {
        self.fail_next_send(
            chat_id,
            StatusCode::BAD_REQUEST,
            "Bad Request: group chat was upgraded to a supergroup chat",
            Some(json!({ "migrate_to_chat_id": to })),
        );
    }

    pub fn rate_limit_next_send(&self, chat_id: i64, retry_after_seconds: u64) {
        self.fail_next_send(
            chat_id,
            StatusCode::TOO_MANY_REQUESTS,
            &format!("Too Many Requests: retry after {retry_after_seconds}"),
            Some(json!({ "retry_after": retry_after_seconds })),
        );
    }
}

async fn handle(
    State(fake): State<FakeTelegram>,
    Path((bot, method)): Path<(String, String)>,
    Json(params): Json<Value>,
) -> Response {
    if bot != format!("bot{TOKEN}") {
        return error(StatusCode::UNAUTHORIZED, "Unauthorized");
    }
    match method.as_str() {
        "getUpdates" => get_updates(&fake, &params).await,
        "sendMessage" => send_message(&fake, &params),
        "setMyCommands" | "setMyDescription" | "setMyShortDescription" => {
            set_profile(&fake, method, params)
        }
        _ => error(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_updates(fake: &FakeTelegram, params: &Value) -> Response {
    let offset = params["offset"].as_i64();
    if let Some(allowed) = params.get("allowed_updates") {
        fake.state.lock().unwrap().allowed_updates = Some(allowed.clone());
    }
    let timeout = Duration::from_secs(params["timeout"].as_u64().unwrap_or(0));
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // Registered before checking, so a push in between still wakes us.
        let pushed = fake.pushed.notified();
        let updates = {
            let mut state = fake.state.lock().unwrap();
            if let Some(offset) = offset {
                state
                    .updates
                    .retain(|update| update["update_id"].as_i64().unwrap() >= offset);
            }
            state.updates.clone()
        };
        if !updates.is_empty() || tokio::time::Instant::now() >= deadline {
            return ok(Value::Array(updates));
        }
        tokio::select! {
            () = pushed => {}
            () = tokio::time::sleep_until(deadline) => {}
        }
    }
}

fn send_message(fake: &FakeTelegram, params: &Value) -> Response {
    let (Some(chat_id), Some(text)) = (params["chat_id"].as_i64(), params["text"].as_str()) else {
        return error(StatusCode::BAD_REQUEST, "Bad Request: missing parameters");
    };
    let mut state = fake.state.lock().unwrap();
    if let Some((status, body)) = state
        .failures
        .get_mut(&chat_id)
        .and_then(VecDeque::pop_front)
    {
        return (status, Json(body)).into_response();
    }
    state.sent.push(Sent {
        chat_id,
        text: text.to_owned(),
    });
    state.sent_at.push(Instant::now());
    let message_id = state.sent.len();
    ok(json!({
        "message_id": message_id,
        "date": 0,
        "chat": { "id": chat_id, "type": "private" },
        "text": text,
    }))
}

fn set_profile(fake: &FakeTelegram, method: String, params: Value) -> Response {
    let mut state = fake.state.lock().unwrap();
    if state.failing_profile_methods.contains(&method) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
    }
    state.profile.push(ProfileCall { method, params });
    ok(Value::Bool(true))
}

fn ok(result: Value) -> Response {
    Json(json!({ "ok": true, "result": result })).into_response()
}

fn error(status: StatusCode, description: &str) -> Response {
    let body = json!({
        "ok": false,
        "error_code": status.as_u16(),
        "description": description,
    });
    (status, Json(body)).into_response()
}

pub async fn serving(router: Router) -> Url {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Url::parse(&format!("http://{address}")).unwrap()
}
