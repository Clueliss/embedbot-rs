use crate::scraper::PostCommonData;
use std::borrow::Cow;

const EMBED_CONTENT_MAX_LEN: usize = 2048;
const EMBED_TITLE_MAX_LEN: usize = 256;

fn find_markdown(text: &[u8]) -> Option<usize> {
    let pos = [
        memx::memchr_qpl(text, b'`', b'*', b'_', b'{'),
        memx::memchr_qpl(text, b'}', b'[', b']', b'('),
        memx::memchr_qpl(text, b')', b'#', b'+', b'-'),
        memx::memchr_dbl(text, b'.', b'!'),
    ];

    pos.into_iter().flatten().min()
}

fn escape_at<'b>(out: &mut Vec<u8>, bytes: &mut &'b [u8], pos: usize) {
    out.extend_from_slice(&bytes[..pos]);
    out.push(b'\\');
    out.push(bytes[pos]);

    *bytes = &bytes[pos + 1..]; // slice[slice.len()..] returns the empty slice
}

pub fn escape_markdown(text: &str) -> Cow<'_, str> {
    let mut bytes = text.as_bytes();

    let Some(first_pos) = find_markdown(bytes) else {
        // nothing to escape
        return Cow::Borrowed(text);
    };

    let mut out = Vec::new();
    escape_at(&mut out, &mut bytes, first_pos);

    while !bytes.is_empty() {
        match find_markdown(bytes) {
            Some(pos) => {
                escape_at(&mut out, &mut bytes, pos);
            },
            None => {
                out.extend_from_slice(bytes);
                break;
            },
        }
    }

    Cow::Owned(String::from_utf8(out).expect("Only inserted backslashes, it should still be UTF-8"))
}

pub fn limit_len(text: &str, limit: usize) -> Cow<'_, str> {
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

pub fn limit_descr_len(text: &str) -> Cow<'_, str> {
    limit_len(text, EMBED_CONTENT_MAX_LEN)
}

#[cfg(test)]
mod tests {
    use super::escape_markdown;
    use std::borrow::Cow;

    #[test]
    fn escape_markdown_sanity_check() {
        let md = "# Hello World\n- First\n- Second+";
        let res = escape_markdown(md);
        assert!(matches!(res, Cow::Owned(_)));
        assert_eq!(res, "\\# Hello World\n\\- First\n\\- Second\\+");
    }

    #[test]
    fn escape_markdown_nothing_to_escape() {
        let non_md = "Hello World";
        let res = escape_markdown(non_md);
        assert!(matches!(res, Cow::Borrowed(_)));
        assert_eq!(res, non_md);
    }
}
