use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::{
    BotCommand, ChatId, ChatMemberStatus, Client, Message, RequestError, Subscriber, Subscribers,
    UpdateKind, User, text,
};
use crate::database::DatabaseError;
use crate::error::with_sources;
use crate::lang::Lang;
use crate::poller::Latest;

const LONG_POLL: Duration = Duration::from_secs(30);
const RETRY_AFTER: Duration = Duration::from_secs(5);

/// Answers commands until the task is aborted.
pub fn start(
    client: Client,
    username: String,
    subscribers: Subscribers,
    latest: watch::Receiver<Latest>,
) -> JoinHandle<()> {
    tokio::spawn(introduce(client.clone()));
    tokio::spawn(async move {
        let bot = Bot {
            client,
            subscribers,
            latest,
            username,
        };
        let mut offset = None;
        loop {
            match bot.client.get_updates(offset, LONG_POLL).await {
                Ok(updates) => {
                    for update in updates {
                        // Confirmed by the next poll, so an update is never
                        // handled twice unless the service restarts mid-batch.
                        offset = Some(update.id + 1);
                        bot.handle(update.kind).await;
                    }
                }
                Err(error) => {
                    eprintln!("receiving Telegram updates: {}", with_sources(&error));
                    tokio::time::sleep(retry_after(&error)).await;
                }
            }
        }
    })
}

async fn introduce(client: Client) {
    for (language_code, lang) in [(None, Lang::En), (Some(Lang::Es.code()), Lang::Es)] {
        let replies = replies(lang);
        let commands = Command::ALL.map(|command| BotCommand {
            command: command.name(),
            description: (replies.command)(command),
        });
        let set = [
            (
                "commands",
                client.set_my_commands(&commands, language_code).await,
            ),
            (
                "description",
                client
                    .set_my_description(replies.description, language_code)
                    .await,
            ),
            (
                "short description",
                client
                    .set_my_short_description(replies.short_description, language_code)
                    .await,
            ),
        ];
        for (what, result) in set {
            if let Err(error) = result {
                eprintln!("setting the bot's {what}: {}", with_sources(&error));
            }
        }
    }
}

fn retry_after(error: &RequestError) -> Duration {
    match error {
        RequestError::RateLimited { retry_after } => *retry_after,
        _ => RETRY_AFTER,
    }
}

struct Bot {
    client: Client,
    subscribers: Subscribers,
    latest: watch::Receiver<Latest>,
    username: String,
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Start,
    Status,
    Subscribe,
    Unsubscribe,
}

impl Command {
    const ALL: [Command; 4] = [
        Command::Start,
        Command::Status,
        Command::Subscribe,
        Command::Unsubscribe,
    ];

    fn name(self) -> &'static str {
        match self {
            Command::Start => "start",
            Command::Status => "status",
            Command::Subscribe => "subscribe",
            Command::Unsubscribe => "unsubscribe",
        }
    }

    fn named(name: &str) -> Option<Command> {
        match name {
            // What /unsubscribe was called before; earlier replies mention it.
            "stop" => Some(Command::Unsubscribe),
            _ => Command::ALL
                .into_iter()
                .find(|command| command.name() == name),
        }
    }
}

impl Bot {
    async fn handle(&self, update: UpdateKind) {
        match update {
            UpdateKind::Message(message) | UpdateKind::ChannelPost(message) => {
                self.answer(message).await;
            }
            UpdateKind::MyChatMember(member)
                if matches!(
                    member.status,
                    ChatMemberStatus::Left | ChatMemberStatus::Kicked
                ) =>
            {
                if let Err(error) = self.subscribers.remove(member.chat.id).await {
                    eprintln!(
                        "unsubscribing a chat that removed the bot: {}",
                        with_sources(&error)
                    );
                }
            }
            UpdateKind::MyChatMember(_) | UpdateKind::Other => {}
        }
    }

    async fn answer(&self, message: Message) {
        let Some(command) = message.text.as_deref().and_then(|text| self.command(text)) else {
            return;
        };
        let chat = message.chat.id;
        let reply = match self.reply(command, chat, message.from.as_ref()).await {
            Ok(reply) => reply,
            Err(error) => {
                eprintln!("answering {command:?}: {}", with_sources(&error));
                return;
            }
        };
        if let Err(error) = self.client.send_message(chat, &reply).await {
            eprintln!("replying to {command:?}: {}", with_sources(&error));
        }
    }

    fn command(&self, text: &str) -> Option<Command> {
        let word = text.split_whitespace().next()?.strip_prefix('/')?;
        let (name, addressee) = match word.split_once('@') {
            Some((name, addressee)) => (name, Some(addressee)),
            None => (word, None),
        };
        if addressee.is_some_and(|addressee| !addressee.eq_ignore_ascii_case(&self.username)) {
            return None;
        }
        Command::named(name)
    }

    async fn reply(
        &self,
        command: Command,
        chat: ChatId,
        from: Option<&User>,
    ) -> Result<String, DatabaseError> {
        let sender_lang = sender_lang(from);
        Ok(match command {
            Command::Start => {
                let lang = self.chat_lang(chat, sender_lang).await?;
                format!("{}\n\n{}", replies(lang).description, self.status(lang))
            }
            Command::Subscribe => {
                self.subscribers
                    .add(Subscriber {
                        chat,
                        lang: sender_lang,
                    })
                    .await?;
                let replies = replies(sender_lang);
                format!(
                    "{}\n\n{}\n\n{}",
                    replies.subscribed,
                    self.status(sender_lang),
                    (replies.stored)(chat)
                )
            }
            Command::Unsubscribe => {
                let lang = self.chat_lang(chat, sender_lang).await?;
                self.subscribers.remove(chat).await?;
                replies(lang).stopped.to_owned()
            }
            Command::Status => self.status(self.chat_lang(chat, sender_lang).await?),
        })
    }

    async fn chat_lang(&self, chat: ChatId, sender_lang: Lang) -> Result<Lang, DatabaseError> {
        Ok(self.subscribers.lang(chat).await?.unwrap_or(sender_lang))
    }

    fn status(&self, lang: Lang) -> String {
        match &*self.latest.borrow() {
            Ok(fetched) => text::status(lang, &fetched.status),
            Err(_) => replies(lang).not_known_yet.to_owned(),
        }
    }
}

fn sender_lang(from: Option<&User>) -> Lang {
    from.and_then(|user| user.language_code.as_deref())
        .and_then(Lang::from_tag)
        .unwrap_or(Lang::En)
}

struct Replies {
    command: fn(Command) -> &'static str,
    description: &'static str,
    short_description: &'static str,
    subscribed: &'static str,
    stored: fn(ChatId) -> String,
    stopped: &'static str,
    not_known_yet: &'static str,
}

fn replies(lang: Lang) -> &'static Replies {
    match lang {
        Lang::Es => &ES,
        Lang::En => &EN,
    }
}

const ES: Replies = Replies {
    command: |command| match command {
        Command::Start => "Qué hace este bot",
        Command::Status => "¿Está abierto el Retiro ahora?",
        Command::Subscribe => "Recibe un aviso cuando el Retiro cierre o vuelva a abrir",
        Command::Unsubscribe => "Deja de recibir avisos",
    },
    description: "Te avisa cuando el parque del Retiro de Madrid cierra o vuelve a abrir, \
                  a partir de los datos del Ayuntamiento. Envía /subscribe para recibir avisos, \
                  /status para consultarlo ahora y /unsubscribe para dejar de recibirlos.",
    short_description: "Te avisa cuando el Retiro cierra o vuelve a abrir.",
    subscribed: "Te avisaremos cuando el Retiro cierre o vuelva a abrir.",
    stored: |chat| {
        format!(
            "Para avisarte guardamos el ID de este chat ({}) y el idioma (español). \
             /unsubscribe deja de avisarte y los borra.",
            chat.0
        )
    },
    stopped: "Ya no te avisaremos. Hemos borrado el ID de este chat y el idioma.",
    not_known_yet: "Todavía no sabemos si el Retiro está abierto. Prueba de nuevo en unos minutos.",
};

const EN: Replies = Replies {
    command: |command| match command {
        Command::Start => "What this bot does",
        Command::Status => "Is Retiro open now?",
        Command::Subscribe => "Get notified when Retiro closes or reopens",
        Command::Unsubscribe => "Stop the notifications",
    },
    description: "Tells you when Madrid’s Retiro Park closes or reopens, based on City Council \
                  data. Send /subscribe to get notified, /status to check now and /unsubscribe \
                  to stop.",
    short_description: "Notifies you when Retiro Park closes or reopens.",
    subscribed: "You’ll be notified when Retiro closes or reopens.",
    stored: |chat| {
        format!(
            "To notify you, we store this chat’s ID ({}) and language (English). \
             /unsubscribe ends the notifications and deletes both.",
            chat.0
        )
    },
    stopped: "You won’t be notified any more. This chat’s ID and language have been deleted.",
    not_known_yet: "We don’t know yet whether Retiro is open. Try again in a few minutes.",
};
