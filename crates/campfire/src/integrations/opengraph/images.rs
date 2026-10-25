//! Inline image links (README, "Known differences"): a URL that points at an image unfurls as the
//! image itself, so the composer replaces the pasted link with a remote image attachment rather
//! than the OpenGraph embed. Two URLs become one:
//!
//! - a path whose last segment carries an image extension (`media.giphy.com/media/X/giphy.gif`);
//! - a `giphy.com/gifs/<slug>` page URL, whose slug's trailing `-`-delimited segment is the GIPHY
//!   media ID (`https://media.giphy.com/media/<ID>/giphy.gif`).
//!
//! The Rails reference unfurled neither: `Location#read_html` refuses to fetch URLs it matches as
//! files and media, and a GIPHY page's `og:image` (when the bots-allowed page could be read at all)
//! would have landed in the embed's small image box. Every fetch here goes through `Location`,
//! with the same private-address guard and redirect handling as `Metadata`, and a candidate whose
//! HEAD isn't a JPEG, PNG, GIF or WebP leaves the URL to the regular OpenGraph unfurl.

use campfire_richtext::uri::{self, Uri, UriError};
use rails_compat::json;

use super::location::Location;
use super::{UnfurlError, metadata};
use crate::integrations::net::Network;

/// The extensions a URL's path may carry for it to unfurl as its image. Not the renderer's
/// restriction: whatever the HEAD's content type was, the composer inserts an image and the
/// browser renders it or shows a broken image marker.
const IMAGE_EXTENSIONS: [&str; 5] = ["gif", "jpg", "jpeg", "png", "webp"];

/// The media URL to unfurl, when `url` points at an image directly or resolves to one.
pub async fn from_url(net: &Network, url: &str) -> Result<Option<String>, UnfurlError> {
    let parsed = match uri::parse(url) {
        Ok(parsed) => parsed,
        // The same errors as in decode order `Metadata::from_url` treats the two differently.
        Err(UriError::InvalidUri) => return Ok(None),
        Err(UriError::InvalidComponent) => return Err(UnfurlError::Raised("URI::InvalidComponentError")),
    };
    let Some(candidate) = direct_media_url(&parsed).or_else(|| giphy_media_url(&parsed)) else { return Ok(None) };
    Ok(valid_media(net, &candidate).await)
}

/// `render json: opengraph` for an image unfurl: only the media URL, and no `title`/`url` to tell
/// the composer's OpenGraph branch apart from an image to insert inline.
pub fn response_json(image: &str) -> String {
    format!(r#"{{"image":{}}}"#, json::encode(image))
}

/// The URL itself, when its path's last segment carries an image extension. A query string reads
/// after the path (`Uri` keeps it in `query`), so `/x/gif` URLs and `/a.png?b=c` behave alike.
fn direct_media_url(parsed: &Uri) -> Option<String> {
    if !parsed.is_http() {
        return None;
    }
    let segment = parsed.path.as_deref()?.rsplit('/').next()?;
    let (_, extension) = segment.rsplit_once('.')?;
    if IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) { Some(parsed.to_s()) } else { None }
}

/// The media URL a GIPHY page URL synthesizes, with the page's scheme. Media IDs are the slug's
/// trailing `-`-delimited segment (`https://giphy.com/gifs/funny-cat-l0MYt5jPR6QXJDpbM`); a
/// single-segment slug (`/gifs/<ID>`) is the ID itself. Any other host or shape isn't a page link.
fn giphy_media_url(parsed: &Uri) -> Option<String> {
    if !matches!(parsed.host.as_deref().map(str::to_ascii_lowercase).as_deref(), Some("giphy.com" | "www.giphy.com")) {
        return None;
    }
    let slug = parsed.path.as_deref()?.strip_prefix("/gifs/")?;
    if slug.is_empty() {
        return None;
    }
    let id = slug.rsplit('-').next()?;
    if id.is_empty() {
        return None;
    }
    Some(format!("{}://media.giphy.com/media/{id}/giphy.gif", parsed.scheme.as_deref()?.to_ascii_lowercase()))
}

/// `valid_image_content_type` over the candidate: a valid `Location` whose HEAD serves one of the
/// metadata allowlist's image types. A site that disallows robots answers with a non-image type
/// and the URL falls back to OpenGraph unfurling (which refuses to fetch it, matching the reference).
async fn valid_media(net: &Network, url: &str) -> Option<String> {
    let content_type = Location::new(net, Some(url)).fetch_content_type().await?.to_lowercase();
    metadata::ALLOWED_IMAGE_CONTENT_TYPES.contains(&content_type.as_str()).then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `direct_media_url`: the path's extension decides, whatever the query has to say, and only
    /// on an http(s) URL with a host.
    #[test]
    fn recognizes_image_paths() {
        let candidate = |url: &str| uri::parse(url).ok().and_then(|parsed| direct_media_url(&parsed));
        assert_eq!(candidate("https://media.giphy.com/media/x/giphy.gif").as_deref(), Some("https://media.giphy.com/media/x/giphy.gif"));
        assert_eq!(candidate("https://media.giphy.com/media/x/giphy.GIF").as_deref(), Some("https://media.giphy.com/media/x/giphy.GIF"));
        assert_eq!(candidate("https://media.giphy.com/media/x/me.png?v=1").as_deref(), Some("https://media.giphy.com/media/x/me.png?v=1"));
        assert_eq!(candidate("https://media.giphy.com/media/x/giphy.mp4"), None);
        assert_eq!(candidate("https://a.example/png"), None);
        assert_eq!(candidate("example.com/a.png"), None);
    }

    /// `giphy_media_url`: the `/gifs/<slug>` page form only, with or without an id-intro hyphen,
    /// keeping the page's scheme.
    #[test]
    fn resolves_giphy_page_slugs() {
        let candidate = |url: &str| uri::parse(url).ok().and_then(|parsed| giphy_media_url(&parsed));
        let giphy = |id: &str| format!("https://media.giphy.com/media/{id}/giphy.gif");
        assert_eq!(candidate("https://giphy.com/gifs/cat-l0MYt5jPR6QXJDpbM"), Some(giphy("l0MYt5jPR6QXJDpbM")));
        assert_eq!(candidate("https://giphy.com/gifs/l0MYt5jPR6QXJDpbM"), Some(giphy("l0MYt5jPR6QXJDpbM")));
        assert_eq!(candidate("http://giphy.com/gifs/x-id"), Some("http://media.giphy.com/media/id/giphy.gif".into()));
        assert_eq!(candidate("https://media.giphy.com/media/x/giphy.gif"), None);
        assert_eq!(candidate("https://giphy.com/search/cat"), None);
        assert_eq!(candidate("https://tenor.com/view/cat-gif"), None);
    }

    /// The shared parsing: a mailto URL raises like `Metadata` would, an unparsable one doesn't.
    #[tokio::test]
    async fn raises_and_passes_through_like_metadata() {
        let net = Network::system();
        assert_eq!(from_url(&net, "mailto:foo").await.map(|_| ()).unwrap_err().to_string(), "raised URI::InvalidComponentError");
        assert_eq!(from_url(&net, "space  foo").await, Ok(None));
    }
}
