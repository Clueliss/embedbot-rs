use reqwest::IntoUrl;
use url::Url;

const USER_AGENT: &str = concat!("github.com/liss-h/embedbot-rs embedbot/", clap::crate_version!());

pub async fn wget<U: IntoUrl>(url: U) -> anyhow::Result<reqwest::Response> {
    let client = reqwest::Client::new();
    client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(Into::into)
}

pub async fn wget_json<U: IntoUrl>(url: U) -> anyhow::Result<serde_json::Value> {
    wget(url).await?.json().await.map_err(Into::into)
}

pub fn url_path_ends_with(haystack: &Url, needle: &str) -> bool {
    haystack.path().trim_end_matches('/').ends_with(needle)
}

pub fn url_path_ends_with_image_extension(haystack: &Url) -> bool {
    const EXTENSIONS: [&str; 11] = [
        ".jpg", ".png", ".gif", ".tif", ".bmp", ".dib", ".jpeg", ".jpe", ".jfif", ".tiff", ".heic",
    ];

    let s = haystack.path().trim_end_matches('/');

    EXTENSIONS.iter().any(|x| s.ends_with(x))
}

pub fn unescape_html(html: &str) -> String {
    html.replace("&amp;", "&")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&quot;", "\"")
}

pub fn unescape_url(url: &str) -> String {
    url.replace("&amp;", "&")
}
