pub mod create_response;
mod util;

use crate::{
    embed_bot::embed::create_response::CreateResponse,
    scraper::{Comment, Post, PostCommonData, PostSpecializedData},
};
use serenity::{
    all::User,
    builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter},
};
use url::Url;

#[derive(Debug, Default)]
pub struct EmbedOptions<'comment> {
    pub comment: Option<&'comment str>,
    pub embed_nsfw: bool,
    pub embed_spoiler: bool,
}

fn include_comment(e: CreateEmbed, comment: &Comment) -> CreateEmbed {
    let name = format!("Comment by {author}", author = comment.author);
    e.field(name, util::escape_markdown(&comment.text), true)
}

fn include_author_comment(e: CreateEmbed, u: &User, comment: &str) -> CreateEmbed {
    let title = format!("Comment by {author}", author = u.display_name());
    e.field(title, comment, false)
}

fn base_embed(author: &User, comment: Option<&str>, post: &PostCommonData) -> CreateEmbed {
    let mut e = CreateEmbed::new()
        .title(util::fmt_title(post))
        .description(util::limit_descr_len(&post.text))
        .author(CreateEmbedAuthor::new(author.display_name()))
        .url(post.src.as_str());

    if let Some(comment) = comment {
        e = include_author_comment(e, author, comment);
    }

    if let Some(comment) = &post.comment {
        e = include_comment(e, comment);
    }

    e
}

fn manual_embed(author: &User, discord_comment: Option<&str>, post: &PostCommonData, embed_urls: &[Url]) -> String {
    let discord_comment = discord_comment
        .map(|c| {
            format!(
                "**Comment By {author}:**\n{comment}\n\n",
                author = author.display_name(),
                comment = c
            )
        })
        .unwrap_or_default();

    let post_comment = post
        .comment
        .as_ref()
        .map(|c| {
            format!(
                "**Comment By {author}:**\n{comment}\n\n",
                author = c.author,
                comment = util::escape_markdown(&c.text)
            )
        })
        .unwrap_or_default();

    let urls = itertools::intersperse(embed_urls.iter().map(Url::as_str), "\n").collect::<String>();

    format!(
        ">>> **{author}**\nSource: <{src}>\nEmbedURL: {embed_url}\n\n{discord_comment}{post_comment}{title}\n\n{text}",
        author = author.display_name(),
        src = &post.src,
        embed_url = urls,
        title = util::fmt_title(post),
        text = util::limit_descr_len(&post.text),
        discord_comment = discord_comment,
        post_comment = post_comment,
    )
}

pub fn embed<R: CreateResponse>(post: &Post, user: &User, opts: &EmbedOptions) -> R {
    let response = R::default();

    if post.common.nsfw && !opts.embed_nsfw {
        response.embed({
            let mut e = CreateEmbed::new()
                .title(util::fmt_title(&post.common))
                .description("Warning NSFW: Click to view content")
                .author(CreateEmbedAuthor::new(user.display_name()))
                .url(post.common.src.as_str());

            if let Some(comment) = &opts.comment {
                e = include_author_comment(e, user, comment);
            }

            e
        })
    } else if post.common.spoiler && !opts.embed_spoiler {
        response.embed({
            let mut e = CreateEmbed::new()
                .title(util::fmt_title(&post.common))
                .description("Spoiler: Click to view content")
                .author(CreateEmbedAuthor::new(user.display_name()))
                .url(post.common.src.as_str());

            if let Some(comment) = &opts.comment {
                e = include_author_comment(e, user, comment);
            }

            if let Some(comment) = &post.common.comment {
                e = include_comment(e, comment);
            }

            e
        })
    } else {
        match &post.specialized {
            PostSpecializedData::TextOnly => response.embed(base_embed(user, opts.comment, &post.common)),
            PostSpecializedData::Image { img_url } => {
                response.embed(base_embed(user, opts.comment, &post.common).image(img_url.as_str()))
            },
            PostSpecializedData::Gallery { img_urls } => {
                response.content(manual_embed(user, opts.comment, &post.common, img_urls))
            },
            PostSpecializedData::Video { video_url } => {
                response.content(manual_embed(user, opts.comment, &post.common, &[video_url.clone()]))
            },
            PostSpecializedData::VideoThumbnail { thumbnail_url } => response.embed(
                base_embed(user, opts.comment, &post.common)
                    .image(thumbnail_url.as_str())
                    .footer(CreateEmbedFooter::new(
                        "This was originally a video. Click title to watch on website.",
                    )),
            ),
        }
    }
}

pub fn error<R: CreateResponse, S: Into<String>>(msg: S) -> R {
    R::default().embed(CreateEmbed::new().title(":x: Error").description(msg))
}
