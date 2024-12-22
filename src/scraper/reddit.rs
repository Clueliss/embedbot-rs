#![cfg(feature = "reddit")]

use crate::scraper::{
    util::{unescape_html, unescape_url, url_path_ends_with, url_path_ends_with_image_extension, wget, wget_json},
    Comment, Post, PostCommonData, PostScraper, PostSpecializedData,
};
use json_nav::json_nav;
use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::async_trait;
use std::{borrow::Cow, convert::TryInto};
use url::Url;

async fn find_canonical_post_url<U: IntoUrl>(post_url: U) -> anyhow::Result<Url> {
    let url = post_url.into_url()?;

    match wget(url.clone()).await {
        Ok(resp) if resp.url().path() != "/over18" => Ok(resp.url().to_owned()),
        _ => Ok(url),
    }
}

fn fmt_title<'t>(title: &'t str, flair: &str) -> Cow<'t, str> {
    if flair.is_empty() {
        Cow::Borrowed(title)
    } else {
        Cow::Owned(format!("{title} [{flair}]"))
    }
}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ApiSettings {}

pub struct Api;

impl Api {
    pub fn from_settings(_settings: ApiSettings) -> Self {
        Self
    }

    fn analyze_post(url: Url, json: &Value) -> anyhow::Result<Post> {
        let top_level_post = json_nav! {
            json => 0 => "data" => "children" => 0 => "data";
            as object
        }?;

        let title = json_nav! { top_level_post => "title"; as str }?.to_string();

        let subreddit = json_nav! { top_level_post => "subreddit"; as str }?.to_string();

        // post_json is either top_level_post or the original post (in case of crosspost)
        let (is_xpost, post_json) = json_nav! { top_level_post => "crosspost_parent_list" => 0; as object }
            .map(|parent| (true, parent))
            .unwrap_or((false, top_level_post));

        let original_subreddit = json_nav! { post_json => "subreddit"; as str }?.to_string();

        let text = unescape_html(json_nav! { post_json => "selftext"; as str }?);

        let flair = json_nav! { post_json => "link_flair_text"; as str }
            .map(ToString::to_string)
            .unwrap_or_default();

        let nsfw = json_nav! { post_json => "over_18"; as bool }.unwrap_or_default();

        let spoiler = json_nav! { post_json => "spoiler"; as bool }.unwrap_or_default();

        let comment = {
            let comment_json = json_nav! {
                json => 1 => "data" => "children" => 0 => "data"
            };

            match comment_json {
                Ok(comment) if url_path_ends_with(&url, json_nav! { comment => "id"; as str }?) => Some(Comment {
                    author: json_nav! { comment => "author"; as str }?.to_owned(),
                    text: unescape_html(json_nav! { comment => "body"; as str }?),
                }),
                _ => None,
            }
        };

        let common_data = PostCommonData {
            src: url,
            origin: if is_xpost {
                format!("reddit.com/r/{subreddit} [XPosted from r/{original_subreddit}]")
            } else {
                format!("reddit.com/r/{subreddit}")
            },
            title: fmt_title(&title, &flair).into_owned(),
            nsfw,
            spoiler,
            text,
            comment,
        };

        // embed_url can be "default" when the original post (referenced by crosspost) is deleted
        let alt_embed_url = json_nav! { top_level_post => "thumbnail"; as str }
            .map_err(anyhow::Error::from)
            .and_then(|s| Url::parse(s).map_err(anyhow::Error::from));

        let specialized_data = match post_json.get("secure_media") {
            Some(Value::Object(sm)) if sm.contains_key("reddit_video") => PostSpecializedData::Video {
                video_url: json_nav! { sm => "reddit_video" => "fallback_url"; as str }?.try_into()?,
            },

            Some(Value::Object(sm)) if sm.contains_key("oembed") => PostSpecializedData::Image {
                img_url: json_nav! { sm => "oembed" => "thumbnail_url"; as str }?
                    .try_into()
                    .unwrap_or(alt_embed_url?),
            },

            _ => {
                if let Some(Value::Object(meta)) = post_json.get("media_metadata") {
                    let mut urls = meta
                        .iter()
                        .map(|(_key, imgmeta)| {
                            (json_nav! { imgmeta => "s" => "u"; as str })
                                .map(unescape_url)
                                .map_err(anyhow::Error::from)
                                .and_then(|u| Url::parse(&u).map_err(anyhow::Error::from))
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| json_nav::JsonNavError::TypeMismatch { expected: "url" })?;

                    if urls.len() == 1 {
                        PostSpecializedData::Image { img_url: urls.pop().unwrap() }
                    } else {
                        PostSpecializedData::Gallery { img_urls: urls }
                    }
                } else {
                    let url = Url::parse(json_nav! { post_json => "url"; as str }?).or(alt_embed_url);

                    match url {
                        Ok(url) if url_path_ends_with_image_extension(&url) => {
                            PostSpecializedData::Image { img_url: url }
                        },
                        Ok(url) if url_path_ends_with(&url, ".gifv") => PostSpecializedData::Video { video_url: url },
                        _ => PostSpecializedData::TextOnly,
                    }
                }
            },
        };

        Ok(Post { common: common_data, specialized: specialized_data })
    }
}

#[async_trait]
impl PostScraper for Api {
    fn is_suitable(&self, url: &Url) -> bool {
        ["reddit.com", "www.reddit.com"].map(Some).contains(&url.domain())
    }

    async fn scrape_post(&self, url: Url) -> anyhow::Result<Post> {
        let (url, json) = {
            let mut u = find_canonical_post_url(url).await?;
            u.set_query(None);

            let mut get_url = u.clone();
            get_url.set_path(&format!("{}.json", u.path()));

            (u, wget_json(get_url).await?)
        };

        Self::analyze_post(url, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn image_post() {
        const JSON: &str = include_str!("../../test_data/reddit/image.json");
        let json: Value = serde_json::from_str(JSON).unwrap();

        let url = "https://www.reddit.com/r/Awwducational/comments/oi687m/a_very_rare_irrawaddy_dolphin_only_92_are/";
        let post = Api::analyze_post(Url::from_str(url).unwrap(), &json).unwrap();

        let expected = Post {
            common: PostCommonData {
                src: Url::from_str("https://www.reddit.com/r/Awwducational/comments/oi687m/a_very_rare_irrawaddy_dolphin_only_92_are/").unwrap(),
                origin: "reddit.com/r/Awwducational".to_owned(),
                title: "A very rare Irrawaddy Dolphin, only 92 are estimated to still exist. These dolphins have a bulging forehead, short beak, and 12-19 teeth on each side of both jaws. [Not yet verified]".to_owned(),
                text: "".to_owned(),
                nsfw: false,
                spoiler: false,
                comment: None,
            },
            specialized: PostSpecializedData::Image {
                img_url: Url::from_str("https://i.redd.it/bsp1l1vynla71.jpg").unwrap(),
            },
        };

        assert_eq!(expected, post);
    }

    #[tokio::test]
    async fn video_post() {
        const JSON: &str = include_str!("../../test_data/reddit/video.json");
        let json: Value = serde_json::from_str(JSON).unwrap();

        let url = "https://www.reddit.com/r/aww/comments/oi6lfk/mama_cat_wants_her_kitten_to_be_friends_with/";
        let post = Api::analyze_post(Url::from_str(url).unwrap(), &json).unwrap();

        let expected = Post {
            common: PostCommonData {
                src: Url::from_str(
                    "https://www.reddit.com/r/aww/comments/oi6lfk/mama_cat_wants_her_kitten_to_be_friends_with/",
                )
                .unwrap(),
                origin: "reddit.com/r/aww".to_owned(),
                title: "Mama cat wants her kitten to be friends with human baby.".to_owned(),
                text: "".to_owned(),
                nsfw: false,
                spoiler: false,
                comment: None,
            },
            specialized: PostSpecializedData::Video {
                video_url: Url::from_str("https://v.redd.it/jx4ua6lirla71/DASH_1080.mp4?source=fallback").unwrap(),
            },
        };

        assert_eq!(expected, post);
    }

    #[tokio::test]
    async fn gallery_post() {
        const JSON: &str = include_str!("../../test_data/reddit/gallery.json");
        let json: Value = serde_json::from_str(JSON).unwrap();

        let url =
            "https://www.reddit.com/r/watercooling/comments/ohvv5w/lian_li_o11d_xl_with_2x_3090_sli_triple_radiator/";
        let post = Api::analyze_post(Url::from_str(url).unwrap(), &json).unwrap();

        let expected = Post {
            common: PostCommonData {
                src: Url::from_str("https://www.reddit.com/r/watercooling/comments/ohvv5w/lian_li_o11d_xl_with_2x_3090_sli_triple_radiator/").unwrap(),
                origin: "reddit.com/r/watercooling".to_owned(),
                title: "Lian li o11D XL with 2x 3090 SLI triple radiator. done for now will upgrade the motherboard and cpu to threadripper in future. this case is solid! [Build Complete]".to_owned(),
                text: "".to_owned(),
                nsfw: false,
                spoiler: false,
                comment: None,
            },
            specialized: PostSpecializedData::Gallery {
                img_urls: vec![
                    Url::from_str("https://preview.redd.it/nuwtn1ytsha71.jpg?width=3876&format=pjpg&auto=webp&s=7743bf4c3dbdff8e34c5a0a33d5171e4b485e1e5").unwrap(),
                    Url::from_str("https://preview.redd.it/wrro81ytsha71.jpg?width=4000&format=pjpg&auto=webp&s=5f1a86f3783d7ae290f733083b2af4397332c1be").unwrap(),
                ],
            },
        };

        assert_eq!(expected, post);
    }
}
