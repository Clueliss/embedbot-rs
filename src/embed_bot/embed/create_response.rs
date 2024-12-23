use serenity::builder::{CreateEmbed, CreateInteractionResponseMessage, CreateMessage};

pub trait CreateResponse: Default {
    fn content(self, s: impl Into<String>) -> Self;
    fn add_embed(self, e: CreateEmbed) -> Self;
}

macro_rules! impl_create_response {
    ($builder:ty) => {
        impl CreateResponse for $builder {
            fn content(self, s: impl Into<String>) -> Self {
                self.content(s)
            }

            fn add_embed(self, e: CreateEmbed) -> Self {
                self.add_embed(e)
            }
        }
    };
}

impl_create_response!(CreateInteractionResponseMessage);

#[cfg(feature = "implicit-auto-embed")]
impl_create_response!(CreateMessage);
