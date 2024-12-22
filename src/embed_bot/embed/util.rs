use crate::scraper::PostCommonData;
use std::borrow::Cow;

const EMBED_CONTENT_MAX_LEN: usize = 2048;
const EMBED_TITLE_MAX_LEN: usize = 256;

v_escape::new!(
    MarkdownEscape;
    '`' -> "\\`",
    '*' -> "\\*",
    '_' -> "\\_",
    '{' -> "\\{",
    '}' -> "\\}",
    '[' -> "\\[",
    ']' -> "\\]",
    '(' -> "\\(",
    ')' -> "\\)",
    '#' -> "\\#",
    '+' -> "\\+",
    '-' -> "\\-",
    '.' -> "\\.",
    '!' -> "\\!"
);

pub fn escape_markdown(title: &str) -> String {
    MarkdownEscape::new(title.as_bytes()).to_string()
}

pub fn limit_len(text: &str, limit: usize) -> Cow<str> {
    const SHORTENED_MARKER: &str = "[...]";

    if text.len() > limit {
        let shortened_text = &text[..(limit - 1 - SHORTENED_MARKER.len())];
        Cow::Owned(format!("{shortened_text} {SHORTENED_MARKER}"))
    } else {
        Cow::Borrowed(text)
    }
}

pub fn fmt_title(post: &PostCommonData) -> String {
    let title = escape_markdown(&post.title);
    let title = limit_len(&title, EMBED_TITLE_MAX_LEN - 3 - post.origin.len()); // -3 for formatting

    format!("{title} - {origin}", origin = post.origin)
}

pub fn limit_descr_len(text: &str) -> Cow<str> {
    limit_len(text, EMBED_CONTENT_MAX_LEN)
}
