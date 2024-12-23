mod embed_bot;
mod scraper;

use anyhow::Context;
use clap::Parser;
use embed_bot::{Config, EmbedBot};
use serenity::{prelude::GatewayIntents, Client};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
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
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let opts = Opts::parse();

    select! {
        res = entrypoint(opts) => match res {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!("{e:#}");
                ExitCode::FAILURE
            },
        },
        _ = tokio::signal::ctrl_c() => {
            ExitCode::SUCCESS
        },
    }
}

async fn entrypoint(opts: Opts) -> anyhow::Result<()> {
    let config = load_config(&opts.config_path).await.context("Unable to load config")?;

    let embed_bot = {
        let mut e = EmbedBot::from_embed_config(config.embed_behaviour);

        if let Some(modules) = config.modules {
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

    let mut client = Client::builder(&config.discord_token, get_gateway_intents())
        .event_handler(embed_bot)
        .await
        .expect("could not create client");

    client.start().await?;

    Ok(())
}

async fn load_config(path: &Path) -> anyhow::Result<Config> {
    let settings_str = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Unable to open config file at {}", path.display()))?;

    let settings: Config =
        toml::from_str(&settings_str).with_context(|| format!("Unable to parse config file at: {}", path.display()))?;

    Ok(settings)
}
