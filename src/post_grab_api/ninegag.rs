use serenity::async_trait;
use serenity::model::user::User;

use crate::nav_json;

use super::*;

fn fmt_title(p: &NineGagPost) -> String {
    let em = escape_markdown(&p.title);
    let title = limit_len(&em, EMBED_TITLE_MAX_LEN - 12); // -12 for formatting

    format!("'{}' - **9GAG**", title)
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum NineGagPostType {
    Image,
    Video,
}

#[derive(Clone, Debug)]
pub struct NineGagPost {
    src: String,
    title: String,
    embed_url: String,
    post_type: NineGagPostType,
}

impl Post for NineGagPost {
    fn should_embed(&self) -> bool {
        self.post_type != NineGagPostType::Image
    }

    fn create_embed(&self, u: &User, comment: Option<&str>, create_msg: &mut CreateMessage) {
        match self.post_type {
            NineGagPostType::Image => {
                create_msg.embed(|e| e.title(&self.title).url(&self.src).image(&self.embed_url))
            }
            NineGagPostType::Video => {
                let discord_comment = comment
                    .map(|c| {
                        format!(
                            "**Comment By {author}:**\n{comment}\n\n",
                            author = u.name,
                            comment = c
                        )
                    })
                    .unwrap_or_default();

                create_msg.content(format!(
                    ">>> **{author}**\nSource: <{src}>\nEmbedURL: {embed_url}\n\n{discord_comment}{title}",
                    author = u.name,
                    src = &self.src,
                    embed_url = self.embed_url,
                    discord_comment = discord_comment,
                    title = fmt_title(self),
                ))
            }
        };
    }
}

#[derive(Default)]
pub struct NineGagAPI;

#[async_trait]
impl PostScraper for NineGagAPI {
    fn is_suitable(&self, url: &Url) -> bool {
        url.domain() == Some("9gag.com")
    }

    async fn get_post(&self, url: Url) -> Result<Box<dyn Post>, Error> {
        let html = wget_html(url.clone(), USER_AGENT).await?;

        let title: String = {
            let title_selector = scraper::Selector::parse("title").unwrap();
            html.select(&title_selector)
                .next()
                .ok_or(Error::JSONNavErr("could not find title"))?
                .text()
                .collect()
        };

        let build_json: serde_json::Value = {
            let script_selector = scraper::Selector::parse("script").unwrap();

            let script_text: String = html
                .select(&script_selector)
                .find(|elem| elem.text().collect::<String>().contains("JSON.parse"))
                .ok_or(Error::JSONNavErr("could not find json"))?
                .text()
                .collect::<String>()
                .replace("\\", "");

            serde_json::from_str(&script_text[29..(script_text.len() - 3)])?
        };

        let post_json = nav_json! { build_json => "data" => "post"; as object }?;

        let (post_type, embed_url) = match nav_json! { post_json => "type"; as str }? {
            "Photo" => (
                NineGagPostType::Image,
                nav_json! { post_json => "images" => "image700" => "url"; as str }?.to_string(),
            ),

            "Animated" => {
                let imgs = nav_json! { post_json => "images"; as object }?;

                let img_alts = nav_json! { imgs => "image460svwm" }
                    .or_else(|_| nav_json! { imgs => "image460sv" })?;

                (
                    NineGagPostType::Video,
                    nav_json! { img_alts => "url"; as str }?.to_string(),
                )
            }

            _ => (
                NineGagPostType::Video,
                nav_json! { post_json => "vp9Url"; as str }?.to_string(),
            ),
        };

        Ok(Box::new(NineGagPost {
            src: url.to_string(),
            title: title[0..(title.len() - 7)].to_string(), // remove ' - 9GAG' from end
            embed_url,
            post_type,
        }))
    }
}
