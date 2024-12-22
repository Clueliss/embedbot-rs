#![cfg(feature = "twitter")]

use crate::scraper::{Post, PostCommonData, PostScraper, PostSpecializedData};
use headless_chrome::LaunchOptions;
use scraper::Html;
use serde::{Deserialize, Serialize};
use serenity::async_trait;
use std::path::{Path, PathBuf};
use url::Url;

fn wget_rendered_html(url: &Url, chrome_executable: Option<&Path>) -> anyhow::Result<Html> {
    let browser = headless_chrome::Browser::new(
        LaunchOptions::default_builder()
            .path(chrome_executable.map(ToOwned::to_owned))
            .build()
            .unwrap(),
    )?;

    let tab = browser.new_tab()?;
    tab.navigate_to(url.as_str())?;
    tab.wait_until_navigated()?;
    let content = tab.get_content()?;

    Ok(Html::parse_document(&content))
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiSettings {
    pub chrome_executable: Option<PathBuf>,
}

pub struct Api {
    settings: ApiSettings,
}

impl Api {
    pub fn from_settings(settings: ApiSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl PostScraper for Api {
    fn is_suitable(&self, url: &Url) -> bool {
        url.domain() == Some("twitter.com") || url.domain() == Some("x.com")
    }

    async fn scrape_post(&self, url: Url) -> anyhow::Result<Post> {
        let chrome_exec = self.settings.chrome_executable.clone();

        let post = tokio::task::spawn_blocking(move || {
            let author = url
                .path_segments()
                .ok_or_else(|| anyhow::anyhow!("Url missing path"))?
                .next()
                .ok_or_else(|| anyhow::anyhow!("Url missing first path element"))?
                .to_owned();

            let html = wget_rendered_html(&url, chrome_exec.as_deref())?;

            let text = {
                let selector = scraper::Selector::parse(r#"article div[data-testid="tweetText"]"#).unwrap();

                html.select(&selector)
                    .next()
                    .map(|e| e.text().filter(|&s| s != "…").collect())
                    .unwrap_or_default()
            };

            let common = PostCommonData {
                text,
                nsfw: false,
                spoiler: false,
                src: url,
                origin: "twitter.com".to_owned(),
                title: format!("@{author}"),
                comment: None,
            };

            let mut img_urls: Vec<_> = {
                let selector = scraper::Selector::parse(r#"article img[alt]:not([alt=""])"#).unwrap();

                html.select(&selector)
                    .filter_map(|e| e.attr("src"))
                    .filter(|src| src.starts_with("https://pbs.twimg.com/media"))
                    .filter_map(|s| Url::parse(s).ok())
                    .collect()
            };

            let specialized = match img_urls.len() {
                0 => {
                    let selector = scraper::Selector::parse("article video").unwrap();

                    if let Some(video) = html.select(&selector).next() {
                        if matches!(video.attr("type"), Some("video/mp4")) {
                            let src = video.attr("src").unwrap();
                            PostSpecializedData::Video { video_url: Url::parse(src)? }
                        } else {
                            let poster = video.attr("poster").unwrap();
                            PostSpecializedData::VideoThumbnail { thumbnail_url: Url::parse(poster)? }
                        }
                    } else {
                        PostSpecializedData::TextOnly
                    }
                },
                1 => PostSpecializedData::Image { img_url: img_urls.swap_remove(0) },
                _ => PostSpecializedData::Gallery { img_urls },
            };

            Ok::<_, anyhow::Error>(Post { common, specialized })
        })
        .await??;

        Ok(post)
    }
}
