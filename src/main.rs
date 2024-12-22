mod embed_bot;
mod scraper;

use anyhow::Context;
use clap::Parser;
use embed_bot::{EmbedBot, Settings};
use serenity::{prelude::GatewayIntents, Client};
use std::{
    path::{Path, PathBuf},
    process::exit,
};
use tokio::select;

#[cfg(feature = "implicit-auto-embed")]
fn get_gateway_intents() -> GatewayIntents {
    GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT
}

#[cfg(not(feature = "implicit-auto-embed"))]
fn get_gateway_intents() -> GatewayIntents {
    GatewayIntents::empty()
}

#[derive(Parser)]
struct Opts {
    #[clap(long, default_value = "/etc/embedbot.toml")]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let opts = Opts::parse();

    let settings = match load_settings(&opts.config_path) {
        Ok(settings) => {
            tracing::debug!("Loaded config: {settings:#?}");
            settings
        },
        Err(e) => {
            tracing::error!("Unable to load config: {e:#}");
            exit(1);
        },
    };

    let embed_bot = {
        let mut e = EmbedBot::from_settings(settings.embed_behaviour);

        if let Some(modules) = settings.modules {
            #[cfg(feature = "reddit")]
            if let Some(settings) = modules.reddit {
                e.register_api(scraper::reddit::Api::from_settings(settings));
            }

            #[cfg(feature = "ninegag")]
            if let Some(settings) = modules.ninegag {
                e.register_api(scraper::ninegag::Api::from_settings(settings));
            }

            #[cfg(feature = "twitter")]
            if let Some(settings) = modules.twitter {
                e.register_api(scraper::twitter::Api::from_settings(settings));
            }
        }

        e
    };

    let mut client = Client::builder(&settings.discord_token, get_gateway_intents())
        .event_handler(embed_bot)
        .await
        .expect("could not create client");

    select! {
        res = client.start() => if let Err(e) = res {
            tracing::error!("Client error: {:?}", e);
        },
        _ = tokio::signal::ctrl_c() => {
        },
    }
}

fn load_settings(path: &Path) -> anyhow::Result<Settings> {
    let settings_str =
        std::fs::read_to_string(path).with_context(|| format!("Unable to open config file at {}", path.display()))?;

    let settings: Settings =
        toml::from_str(&settings_str).with_context(|| format!("Unable to parse config file at: {}", path.display()))?;

    Ok(settings)
}
