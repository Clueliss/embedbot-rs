use crate::scraper;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub discord_token: String,
    pub embed_behaviour: EmbedBehaviours,
    pub modules: Option<Modules>,
}

impl Debug for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("discord_token", &"[REDACTED]")
            .field("embed_behaviour", &self.embed_behaviour)
            .field("modules", &self.modules)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct EmbedBehaviour {
    pub default: bool,
    pub allow_override: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct EmbedBehaviours {
    pub nsfw: EmbedBehaviour,
    pub spoiler: EmbedBehaviour,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Modules {
    #[cfg(feature = "reddit")]
    pub reddit: Option<scraper::reddit::ApiSettings>,

    #[cfg(feature = "ninegag")]
    pub ninegag: Option<scraper::ninegag::ApiSettings>,

    #[cfg(feature = "twitter")]
    pub twitter: Option<scraper::twitter::ApiSettings>,
}
