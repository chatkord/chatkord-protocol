//! What a message's link preview must be, as the server checks it and a client builds it: the sender's claim about the
//! page one of the body's links names, bounded and shaped, never fetched by the server.

use super::link::LinkTarget;
use super::message::Spans;
use super::{TextRefused, text_shape};
use crate::chatkord_v1::LinkPreview;
use crate::files::FileId;

/// The most Unicode scalar values a preview's title holds.
pub const MAX_PREVIEW_TITLE_CHARS: usize = 200;

/// The most Unicode scalar values a preview's description holds.
pub const MAX_PREVIEW_DESCRIPTION_CHARS: usize = 500;

/// Why a preview was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewRefused {
    /// Its `url` is not the text of a link span of the body, or not an `https://` link without credentials before its
    /// host.
    NotALink,
    /// The title is empty, over [`MAX_PREVIEW_TITLE_CHARS`], or holds a control or direction character.
    Title,
    /// The description is over [`MAX_PREVIEW_DESCRIPTION_CHARS`], or holds a control or direction character.
    Description,
    /// The picture's file id is not [`FileId::LEN`] bytes.
    File,
}

/// A preview proved in shape against the body it belongs to. Made only by [`preview_shape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Previewed {
    url: String,
    title: String,
    description: String,
    file: Option<FileId>,
}

impl Previewed {
    /// The link, which is the text of one of the body's link spans.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The page's title, trimmed.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The page's description, trimmed; empty for none.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The picture's file, to be taken as an attachment is; none for no picture.
    pub fn file(&self) -> Option<FileId> {
        self.file
    }
}

/// The preview `sent` makes for a message of `body` with `spans`: its url the text of one link span, an `https://` link
/// with no credentials before the host; its title 1 to [`MAX_PREVIEW_TITLE_CHARS`] and its description at most
/// [`MAX_PREVIEW_DESCRIPTION_CHARS`] scalar values, trimmed and shaped as a name is; its file, if any, one whole id.
/// Whether the file is the sender's own unsent picture is the store's to say.
pub fn preview_shape(sent: &LinkPreview, body: &str, spans: &Spans) -> Result<Previewed, PreviewRefused> {
    let is_a_span = spans.links(body).contains(&sent.url.as_str());
    let secure = LinkTarget::parse(&sent.url).is_some_and(|link| link.is_https() && !link.has_userinfo());
    if !is_a_span || !secure {
        return Err(PreviewRefused::NotALink);
    }
    let title = text_shape(&sent.title, MAX_PREVIEW_TITLE_CHARS).map_err(|_| PreviewRefused::Title)?;
    let description = match text_shape(&sent.description, MAX_PREVIEW_DESCRIPTION_CHARS) {
        Ok(description) => description,
        Err(TextRefused::Empty) if sent.description.trim().is_empty() => "",
        Err(TextRefused::Empty | TextRefused::ControlCharacter | TextRefused::TooLong) => {
            return Err(PreviewRefused::Description);
        }
    };
    let file = match sent.file.as_deref() {
        None => None,
        Some(bytes) => {
            Some(FileId::from_bytes(<[u8; FileId::LEN]>::try_from(bytes).map_err(|_| PreviewRefused::File)?))
        }
    };
    Ok(Previewed { url: sent.url.clone(), title: title.to_string(), description: description.to_string(), file })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chatkord_v1::{Link, Span, span};
    use crate::rules::spans_shape;

    const URL: &str = "https://example.invalid/a";

    fn body() -> (String, Spans) {
        let body = format!("see {URL} now");
        let spans = spans_shape(&body, &[Span { start: 4, end: 4 + 25, kind: Some(span::Kind::Link(Link {})) }])
            .expect("spans");
        (body, spans)
    }

    fn preview() -> LinkPreview {
        LinkPreview { url: URL.into(), title: " A page ".into(), description: String::new(), file: None }
    }

    fn refused(preview: &LinkPreview) -> Option<PreviewRefused> {
        let (body, spans) = body();
        preview_shape(preview, &body, &spans).err()
    }

    #[test]
    fn a_preview_of_a_link_span_is_trimmed_and_kept() {
        let (body, spans) = body();
        let kept = preview_shape(&preview(), &body, &spans).expect("kept");
        assert_eq!((kept.url(), kept.title(), kept.description(), kept.file()), (URL, "A page", "", None));
        let with_file = LinkPreview { file: Some(vec![7; FileId::LEN]), description: "About it".into(), ..preview() };
        let kept = preview_shape(&with_file, &body, &spans).expect("kept");
        assert_eq!((kept.description(), kept.file()), ("About it", Some(FileId::from_bytes([7; FileId::LEN]))));
    }

    #[test]
    fn a_url_that_is_not_a_span_or_not_https_is_refused() {
        let other = LinkPreview { url: "https://example.invalid/b".into(), ..preview() };
        assert_eq!(refused(&other), Some(PreviewRefused::NotALink), "not a span of the body");
        let prefix = LinkPreview { url: "https://example.invalid".into(), ..preview() };
        assert_eq!(refused(&prefix), Some(PreviewRefused::NotALink), "a part of a span is not the span");
        let plain = "http://example.invalid/a";
        let body = format!("see {plain}");
        let spans =
            spans_shape(&body, &[Span { start: 4, end: 28, kind: Some(span::Kind::Link(Link {})) }]).expect("spans");
        let http = LinkPreview { url: plain.into(), ..preview() };
        assert_eq!(preview_shape(&http, &body, &spans), Err(PreviewRefused::NotALink), "http");
        let tricked = "https://example.invalid@evil.invalid/a";
        let body = format!("see {tricked}");
        let spans =
            spans_shape(&body, &[Span { start: 4, end: 42, kind: Some(span::Kind::Link(Link {})) }]).expect("spans");
        let tricky = LinkPreview { url: tricked.into(), ..preview() };
        assert_eq!(preview_shape(&tricky, &body, &spans), Err(PreviewRefused::NotALink), "credentials");
    }

    #[test]
    fn a_title_and_description_keep_to_their_bounds_and_their_characters() {
        let title = |title: String| refused(&LinkPreview { title, ..preview() });
        assert_eq!(title(String::new()), Some(PreviewRefused::Title));
        assert_eq!(title("  ".into()), Some(PreviewRefused::Title));
        assert_eq!(title("a".repeat(MAX_PREVIEW_TITLE_CHARS + 1)), Some(PreviewRefused::Title));
        assert_eq!(title("a".repeat(MAX_PREVIEW_TITLE_CHARS)), None);
        assert_eq!(title("a\nb".into()), Some(PreviewRefused::Title));
        assert_eq!(title("a\u{202E}b".into()), Some(PreviewRefused::Title));
        let description = |description: String| refused(&LinkPreview { description, ..preview() });
        assert_eq!(description("a".repeat(MAX_PREVIEW_DESCRIPTION_CHARS + 1)), Some(PreviewRefused::Description));
        assert_eq!(description("a".repeat(MAX_PREVIEW_DESCRIPTION_CHARS)), None);
        assert_eq!(description("   ".into()), None, "none, however it is spelled");
        assert_eq!(description("a\u{0}b".into()), Some(PreviewRefused::Description));
        assert_eq!(description("a\u{2066}b".into()), Some(PreviewRefused::Description));
    }

    #[test]
    fn a_file_is_one_whole_id() {
        for bytes in [vec![], vec![1; FileId::LEN - 1], vec![1; FileId::LEN + 1]] {
            assert_eq!(refused(&LinkPreview { file: Some(bytes), ..preview() }), Some(PreviewRefused::File));
        }
    }
}
