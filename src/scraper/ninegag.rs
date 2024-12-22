#![cfg(feature = "ninegag")]

use crate::scraper::{util::wget, Post, PostCommonData, PostScraper, PostSpecializedData};
use json_nav::json_nav;
use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};
use serenity::async_trait;
use url::Url;

async fn wget_html<U: IntoUrl>(url: U) -> anyhow::Result<scraper::Html> {
    let resp = wget(url).await?;
    Ok(scraper::Html::parse_document(&resp.text().await?))
}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ApiSettings {}

pub struct Api;

impl Api {
    pub fn from_settings(_settings: ApiSettings) -> Self {
        Self
    }
}

#[async_trait]
impl PostScraper for Api {
    fn is_suitable(&self, url: &Url) -> bool {
        url.domain() == Some("9gag.com")
    }

    async fn scrape_post(&self, url: Url) -> anyhow::Result<Post> {
        let html = wget_html(url.clone()).await?;

        let title: String = {
            let title_selector = scraper::Selector::parse("title").unwrap();
            html.select(&title_selector)
                .next()
                .ok_or_else(|| anyhow::anyhow!("could not find title"))?
                .text()
                .collect()
        };

        let build_json: serde_json::Value = {
            let script_selector = scraper::Selector::parse("script").unwrap();

            let script_text: String = html
                .select(&script_selector)
                .find(|elem| elem.text().collect::<String>().contains("JSON.parse"))
                .ok_or_else(|| anyhow::anyhow!("could not find json"))?
                .text()
                .collect::<String>()
                .replace('\\', "");

            serde_json::from_str(&script_text[29..(script_text.len() - 3)])?
        };

        let post_json = json_nav! { build_json => "data" => "post"; as object }?;

        let common = PostCommonData {
            src: url,
            origin: "9gag.com".to_string(),
            title,
            text: "".to_string(),
            nsfw: false,
            spoiler: false,
            comment: None,
        };

        let specialized = match json_nav! { post_json => "type"; as str }? {
            "Photo" => {
                let img_url = Url::parse(json_nav! { post_json => "images" => "image700" => "url"; as str }?)?;
                PostSpecializedData::Image { img_url }
            },
            "Animated" => {
                let imgs = json_nav! { post_json => "images"; as object }?;
                let img_alts = json_nav! { imgs => "image460svwm" }.or_else(|_| json_nav! { imgs => "image460sv" })?;
                let gif_url = Url::parse(json_nav! { img_alts => "url"; as str }?)?;

                PostSpecializedData::Video { video_url: gif_url }
            },
            _ => {
                let video_url = Url::parse(json_nav! { post_json => "vp9Url"; as str }?)?;
                PostSpecializedData::Video { video_url }
            },
        };

        Ok(Post { common, specialized })
    }
}
