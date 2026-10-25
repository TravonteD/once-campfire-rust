//! Inline image links (README, "Known differences"): a remote image attachment the composer
//! inserts and edits, which the plain-text and rendering pipelines must show in full. The
//! reference has no such flow: it never unfurls a URL that points at an image, `editable_body`
//! raises for every remote-media attachment, and editing one goes through `ContentAttachment`
//! semantics the editor reserves for its own embed types.

use campfire_richtext::{AttachableResolver, GidLookup, RenderContext, SignedLookup, editable_value, message_presentation, to_plain_text};

struct NoRecords;

impl AttachableResolver for NoRecords {
    fn locate_signed(&self, _sgid: &str) -> SignedLookup {
        SignedLookup::Invalid
    }

    fn find_gid(&self, _gid: &str) -> GidLookup {
        GidLookup::NotFound
    }
}

fn ctx() -> RenderContext<'static> {
    RenderContext { resolver: &NoRecords, request_host: Some("once.campfire.test".into()) }
}

/// What Lexxy writes for a pasted image URL: an attachment whose URL and (editor's) media type
/// name it, without an SGID. Lexxy's `img` import keeps the file name and the `alt` text.
fn image_body(src: &str) -> String {
    format!(
        r#"<action-text-attachment url="{src}" alt="" content-type="image/*" filename="giphy.gif" presentation="gallery"></action-text-attachment>"#
    )
}

#[test]
fn renders_an_inline_image_figure() {
    let html = message_presentation(&image_body("https://media.giphy.com/media/x/giphy.gif"), &ctx()).unwrap();
    assert!(
        html.contains(
            "<figure class=\"attachment attachment--preview\">\n  <img src=\"https://media.giphy.com/media/x/giphy.gif\" />\n</figure>"
        ),
        "{html}"
    );
}

#[test]
fn plain_text_names_the_image() {
    assert_eq!(to_plain_text(&image_body("https://a.example/x.gif"), &ctx()).unwrap(), "[Image]");
}

/// Rails rebuilds every attachment from its attachable and raises for a remote image, which made
/// such a message uneditable; the stored node round-trips instead, and the editor ingests it
/// whole (`url`, `content-type`, `presentation`).
#[test]
fn editing_keeps_the_remote_image_as_stored() {
    let body = image_body("https://media.giphy.com/media/x/giphy.gif");
    assert_eq!(editable_value(&body, &ctx()).unwrap().unwrap(), body);
}
