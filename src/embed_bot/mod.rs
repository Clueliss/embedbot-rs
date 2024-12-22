mod embed;
mod settings;

use crate::{
    embed_bot::{
        embed::EmbedOptions,
        settings::{EmbedBehaviour, EmbedBehaviours},
    },
    scraper::{Post, PostScraper},
};
use itertools::Itertools;
use serenity::{
    async_trait,
    builder::{CreateCommand, CreateCommandOption, CreateInteractionResponse},
    client::{Context, EventHandler},
    model::{
        application::{
            Command, CommandData, CommandDataOption, CommandDataOptionValue, CommandOptionType, CommandType,
            Interaction,
        },
        channel::Message,
        gateway::Ready,
    },
};
pub use settings::Settings;
use thiserror::Error;
use url::Url;

pub struct EmbedBot {
    apis: Vec<Box<dyn PostScraper + Send + Sync>>,
    embed_behaviours: EmbedBehaviours,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("No scraper available")]
    NoScraperAvailable,

    #[error("Unable to scrape post: {0:#}")]
    PostScrapeFailed(#[from] anyhow::Error),
}

impl EmbedBot {
    pub fn from_settings(settings: EmbedBehaviours) -> Self {
        EmbedBot { apis: Vec::new(), embed_behaviours: settings }
    }

    pub fn register_api<T: 'static + PostScraper + Send + Sync>(&mut self, api: T) {
        self.apis.push(Box::new(api));
    }

    fn find_api(&self, url: &Url) -> Option<&(dyn PostScraper + Send + Sync)> {
        self.apis.iter().find(|a| a.is_suitable(url)).map(AsRef::as_ref)
    }

    async fn scrape_post(&self, mut url: Url) -> Result<Post, Error> {
        if let Some(api) = self.find_api(&url) {
            url.set_fragment(None);
            let post = api.scrape_post(url).await?;
            Ok(post)
        } else {
            Err(Error::NoScraperAvailable)
        }
    }
}

macro_rules! server_communication_try {
    ($res:expr, $msg:expr) => {
        match $res {
            Ok(value) => value,
            Err(err) => {
                tracing::error!("{msg}: {err:#}", msg = $msg);
                return;
            },
        }
    };
}

macro_rules! interaction_try {
    ($command:expr, $ctx:expr, $res:expr) => {
        match $res {
            Ok(opt) => opt,
            Err(err) => {
                server_communication_try!(
                    $command
                        .create_response(
                            $ctx,
                            CreateInteractionResponse::Message(embed::error(format!("Invalid input: {err}"))),
                        )
                        .await,
                    "Unable to send error response"
                );

                return;
            },
        }
    };
}

#[async_trait]
impl EventHandler for EmbedBot {
    #[cfg(feature = "implicit-auto-embed")]
    async fn message(&self, ctx: Context, msg: Message) {
        if !msg.author.bot {
            let content: Vec<_> = msg.content.lines().collect();

            let (url, comment) = match &content[..] {
                [] => (None, None),
                [a] => (Url::parse(a).ok(), None),
                args => {
                    let (urls, comments): (Vec<_>, Vec<_>) = args
                        .iter()
                        .filter(|s| !s.is_empty())
                        .partition(|a| Url::parse(a).is_ok());

                    let mut urls = urls.into_iter().map(|u| Url::parse(u).unwrap());

                    let comments: String = Itertools::intersperse(comments.into_iter(), "\n").collect();

                    (urls.next(), Some(comments))
                },
            };

            if let Some(url) = url {
                match self.scrape_post(url.clone()).await {
                    Ok(post) => {
                        server_communication_try!(
                            msg.channel_id
                                .send_message(
                                    &ctx,
                                    embed::embed(
                                        &post,
                                        &msg.author,
                                        &EmbedOptions { comment: comment.as_deref(), ..Default::default() },
                                    ),
                                )
                                .await,
                            "Unable to send message"
                        );

                        server_communication_try!(msg.delete(&ctx).await, "Unable to delete user message");
                    },
                    Err(Error::NoScraperAvailable) => {
                        tracing::info!("not embedding {}: no scraper available", url);
                    },
                    Err(e) => {
                        tracing::error!("error while trying to embed {}: {}", url, e);
                    },
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        server_communication_try!(
            Command::create_global_command(
                &ctx,
                CreateCommand::new("embed")
                    .kind(CommandType::ChatInput)
                    .description("embed a post")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::String, "url", "url of the post").required(true),
                    )
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::Boolean,
                            "embed-nsfw",
                            "embed post fully even if it is flagged as nsfw",
                        )
                        .required(false),
                    )
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::Boolean,
                            "embed-spoiler",
                            "embed post fully even if it is flagged as spoiler",
                        )
                        .required(false),
                    )
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::String, "comment", "a personal comment to include")
                            .required(false),
                    ),
            )
            .await,
            "Unable to set up commands"
        );

        tracing::info!("logged in");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = &interaction {
            match &command.data {
                CommandData { name, options, .. } if name == "embed" => {
                    let url = interaction_try!(
                        &command,
                        &ctx,
                        match parse_option(options, "url", |x| x.as_str()) {
                            Ok(Some(value)) => Ok(value),
                            Ok(None) => Err(anyhow::anyhow!("Parameter embed must be present")),
                            Err(e) => Err(e),
                        }
                    );

                    let comment = interaction_try!(&command, &ctx, parse_option(options, "comment", |x| x.as_str()));

                    let embed_nsfw = select_embed_behaviour(
                        &self.embed_behaviours.nsfw,
                        interaction_try!(&command, &ctx, parse_option(options, "embed-nsfw", |x| x.as_bool())),
                    );

                    let embed_spoiler = select_embed_behaviour(
                        &self.embed_behaviours.spoiler,
                        interaction_try!(&command, &ctx, parse_option(options, "embed-spoiler", |x| x.as_bool())),
                    );

                    let opts = EmbedOptions { comment, embed_nsfw, embed_spoiler };

                    match Url::parse(url) {
                        Ok(url) => {
                            let user = &command.user;

                            match self.scrape_post(url.clone()).await {
                                Ok(post) => {
                                    server_communication_try!(
                                        command
                                            .create_response(
                                                &ctx,
                                                CreateInteractionResponse::Message(embed::embed(&post, user, &opts)),
                                            )
                                            .await,
                                        "Unable to send response"
                                    );

                                    tracing::trace!("embedded '{}': {:?}", url, post);
                                },
                                Err(e) => {
                                    let msg = format!("{}", e);
                                    tracing::error!("error: {msg}");

                                    server_communication_try!(
                                        command
                                            .create_response(
                                                &ctx,
                                                CreateInteractionResponse::Message(embed::error(msg))
                                            )
                                            .await,
                                        "Unable to send error response"
                                    );
                                },
                            }
                        },
                        Err(_) => {
                            server_communication_try!(
                                command
                                    .create_response(
                                        &ctx,
                                        CreateInteractionResponse::Message({
                                            embed::error(format!("Could not parse url: {url}"))
                                        }),
                                    )
                                    .await,
                                "Unable to send error response"
                            );
                        },
                    }
                },
                _ => (),
            }
        }
    }
}

fn parse_option<'val, I, F, T>(options: I, name: &str, try_map_value: F) -> anyhow::Result<Option<T>>
where
    I: IntoIterator<Item = &'val CommandDataOption>,
    F: FnOnce(&'val CommandDataOptionValue) -> Option<T>,
    T: 'val,
{
    let opt = options.into_iter().find(|c| c.name == name);

    match opt {
        Some(CommandDataOption { value, .. }) => match try_map_value(value) {
            Some(value) => Ok(Some(value)),
            None => Err(anyhow::anyhow!(
                "Invalid type for parameter {name}, expected {expected}",
                expected = std::any::type_name::<T>()
            )),
        },
        None => Ok(None),
    }
}

fn select_embed_behaviour(behav: &EmbedBehaviour, requested: Option<bool>) -> bool {
    match requested {
        Some(request) if behav.allow_override => request,
        _ => behav.default,
    }
}
