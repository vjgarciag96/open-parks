use std::fmt;
use std::time::Duration;

use reqwest::{StatusCode, Url, header};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::http;

const API_URL: &str = "https://api.telegram.org";
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct BotToken(String);

impl BotToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }
}

impl BotToken {
    fn path_segment(&self) -> String {
        format!("bot{}", self.0)
    }
}

impl fmt::Debug for BotToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BotToken(..)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    api_url: Url,
    token: BotToken,
    /// Set per request rather than on `http`, since a long poll needs longer.
    timeout: Duration,
}

impl Client {
    pub fn live(token: BotToken) -> Result<Self, reqwest::Error> {
        let api_url = Url::parse(API_URL).expect("API_URL is valid");
        Self::new(api_url, token, TIMEOUT)
    }

    pub fn new(api_url: Url, token: BotToken, timeout: Duration) -> Result<Self, reqwest::Error> {
        let http = http::client_builder().build()?;
        Ok(Self {
            http,
            api_url,
            token,
            timeout,
        })
    }

    pub async fn send_message(&self, chat: ChatId, text: &str) -> Result<(), RequestError> {
        #[derive(Serialize)]
        struct Params<'a> {
            chat_id: ChatId,
            text: &'a str,
        }

        #[derive(Deserialize)]
        struct SentMessage {}

        let SentMessage {} = self
            .call(
                "sendMessage",
                &Params {
                    chat_id: chat,
                    text,
                },
                self.timeout,
            )
            .await?;
        Ok(())
    }

    /// Sets the commands listed to users whose language is `language_code`,
    /// or to everyone else when it's `None`.
    pub async fn set_my_commands(
        &self,
        commands: &[BotCommand],
        language_code: Option<&str>,
    ) -> Result<(), RequestError> {
        #[derive(Serialize)]
        struct Params<'a> {
            commands: &'a [BotCommand],
            #[serde(skip_serializing_if = "Option::is_none")]
            language_code: Option<&'a str>,
        }

        self.set(
            "setMyCommands",
            &Params {
                commands,
                language_code,
            },
        )
        .await
    }

    pub async fn set_my_description(
        &self,
        description: &str,
        language_code: Option<&str>,
    ) -> Result<(), RequestError> {
        #[derive(Serialize)]
        struct Params<'a> {
            description: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            language_code: Option<&'a str>,
        }

        self.set(
            "setMyDescription",
            &Params {
                description,
                language_code,
            },
        )
        .await
    }

    pub async fn set_my_short_description(
        &self,
        short_description: &str,
        language_code: Option<&str>,
    ) -> Result<(), RequestError> {
        #[derive(Serialize)]
        struct Params<'a> {
            short_description: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            language_code: Option<&'a str>,
        }

        self.set(
            "setMyShortDescription",
            &Params {
                short_description,
                language_code,
            },
        )
        .await
    }

    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        long_poll: Duration,
    ) -> Result<Vec<Update>, RequestError> {
        #[derive(Serialize)]
        struct Params {
            #[serde(skip_serializing_if = "Option::is_none")]
            offset: Option<i64>,
            timeout: u64,
            allowed_updates: &'static [&'static str],
        }

        let updates: Vec<RawUpdate> = self
            .call(
                "getUpdates",
                &Params {
                    offset,
                    timeout: long_poll.as_secs(),
                    // Telegram keeps the last list asked for, so one from an
                    // earlier deployment could leave out a kind decoded here.
                    allowed_updates: &["message", "channel_post", "my_chat_member"],
                },
                self.timeout + long_poll,
            )
            .await?;
        Ok(updates.into_iter().map(Update::from).collect())
    }

    async fn set(&self, method: &str, params: &impl Serialize) -> Result<(), RequestError> {
        let _: bool = self.call(method, params, self.timeout).await?;
        Ok(())
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: &impl Serialize,
        timeout: Duration,
    ) -> Result<T, RequestError> {
        let body = serde_json::to_vec(params).expect("parameters serialize");
        let mut response = self
            .http
            .post(self.method_url(method))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .timeout(timeout)
            .send()
            .await?;
        let http_status = response.status();
        let body =
            http::read_body(&mut response, MAX_BODY_BYTES, RequestError::BodyTooLarge).await?;
        let Ok(reply) = serde_json::from_slice::<Reply>(&body) else {
            return Err(RequestError::HttpStatus(http_status));
        };
        match reply {
            Reply {
                ok: true,
                result: Some(result),
                ..
            } => serde_json::from_value(result).map_err(RequestError::Malformed),
            Reply {
                error_code,
                description,
                parameters,
                ..
            } => Err(RequestError::from_reply(
                error_code.unwrap_or(http_status.as_u16()),
                description.unwrap_or_default(),
                parameters.unwrap_or_default(),
            )),
        }
    }

    fn method_url(&self, method: &str) -> Url {
        let mut url = self.api_url.clone();
        url.path_segments_mut()
            .expect("the API URL can be a base")
            .pop_if_empty()
            .push(&self.token.path_segment())
            .push(method);
        url
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BotCommand {
    pub command: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub id: i64,
    pub kind: UpdateKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    Message(Message),
    ChannelPost(Message),
    MyChatMember(ChatMemberUpdated),
    /// Any other kind of update, or one that doesn't decode; it still has
    /// to be confirmed.
    #[serde(skip)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Message {
    pub chat: Chat,
    pub from: Option<User>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Chat {
    pub id: ChatId,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
    pub language_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawChatMemberUpdated")]
pub struct ChatMemberUpdated {
    pub chat: Chat,
    pub status: ChatMemberStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMemberStatus {
    Left,
    Kicked,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawChatMemberUpdated {
    chat: Chat,
    new_chat_member: ChatMember,
}

#[derive(Deserialize)]
struct ChatMember {
    status: ChatMemberStatus,
}

impl From<RawChatMemberUpdated> for ChatMemberUpdated {
    fn from(raw: RawChatMemberUpdated) -> Self {
        ChatMemberUpdated {
            chat: raw.chat,
            status: raw.new_chat_member.status,
        }
    }
}

#[derive(Deserialize)]
struct RawUpdate {
    update_id: i64,
    /// The update's one other field, named after its kind.
    #[serde(flatten)]
    kind: serde_json::Value,
}

impl From<RawUpdate> for Update {
    fn from(raw: RawUpdate) -> Self {
        Update {
            id: raw.update_id,
            kind: serde_json::from_value(raw.kind).unwrap_or(UpdateKind::Other),
        }
    }
}

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    result: Option<serde_json::Value>,
    error_code: Option<u16>,
    description: Option<String>,
    parameters: Option<ResponseParameters>,
}

#[derive(Default, Deserialize)]
struct ResponseParameters {
    migrate_to_chat_id: Option<ChatId>,
    retry_after: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Telegram refused the chat to the bot")]
    Forbidden,
    #[error("Telegram doesn't know the chat")]
    ChatNotFound,
    #[error("the chat migrated to a supergroup")]
    Migrated(ChatId),
    #[error("Telegram asked to retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },
    #[error("Telegram error {code}: {description}")]
    Api { code: u16, description: String },
    #[error("request to Telegram timed out")]
    Timeout,
    #[error("request to Telegram failed")]
    Transport(#[source] reqwest::Error),
    #[error("Telegram responded with HTTP {0}")]
    HttpStatus(StatusCode),
    #[error("Telegram's response is over {MAX_BODY_BYTES} bytes")]
    BodyTooLarge,
    #[error("malformed response from Telegram: {0}")]
    Malformed(serde_json::Error),
}

impl RequestError {
    fn from_reply(code: u16, description: String, parameters: ResponseParameters) -> Self {
        if let Some(chat) = parameters.migrate_to_chat_id {
            return RequestError::Migrated(chat);
        }
        if let Some(seconds) = parameters.retry_after {
            return RequestError::RateLimited {
                retry_after: Duration::from_secs(seconds),
            };
        }
        match code {
            403 => RequestError::Forbidden,
            400 if description.contains("chat not found") => RequestError::ChatNotFound,
            _ => RequestError::Api { code, description },
        }
    }
}

impl From<reqwest::Error> for RequestError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            RequestError::Timeout
        } else {
            // The URL holds the token.
            RequestError::Transport(error.without_url())
        }
    }
}
