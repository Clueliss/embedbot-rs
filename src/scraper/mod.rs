pub mod ninegag;
pub mod reddit;
pub mod twitter;
mod util;

use serenity::async_trait;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub author: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostCommonData {
    pub src: Url,
    pub origin: String,
    pub title: String,
    pub text: String,
    pub nsfw: bool,
    pub spoiler: bool,
    pub comment: Option<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostSpecializedData {
    TextOnly,
    Gallery { img_urls: Vec<Url> },
    Image { img_url: Url },
    Video { video_url: Url },
    VideoThumbnail { thumbnail_url: Url },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub common: PostCommonData,
    pub specialized: PostSpecializedData,
}

#[async_trait]
pub trait PostScraper {
    fn is_suitable(&self, url: &Url) -> bool;
    async fn scrape_post(&self, url: Url) -> anyhow::Result<Post>;
}
