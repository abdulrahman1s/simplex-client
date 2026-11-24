//! A link's preview between the page's `LinkInfo` and the protocol's `LinkPreview`.

use crate::types::model::LinkInfo;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::profile::picture_of;
use crate::chat::content::MsgContent;
use crate::chat::image::ImageData;
use crate::chat::link_preview::LinkPreview;

/// SimpleX's 1x1 PNG, for a preview with no picture.
const NO_IMAGE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

#[must_use]
pub fn to_chat(l: &LinkInfo) -> LinkPreview {
    let image = l.image.as_ref().map_or_else(
        || NO_IMAGE.to_owned(),
        |p| {
            let kind = if p.mime == "image/png" { "png" } else { "jpg" };
            format!("data:image/{kind};base64,{}", STANDARD.encode(&p.bytes))
        },
    );
    LinkPreview {
        uri: l.uri.clone(),
        title: l.title.clone(),
        description: l.description.clone(),
        image: ImageData(image),
        content: None,
    }
}

/// The preview a received message's content has, if it is a link's.
#[must_use]
pub fn of(content: &MsgContent) -> Option<LinkInfo> {
    let MsgContent::Link { preview, .. } = content else {
        return None;
    };
    let image = (preview.image.0 != NO_IMAGE)
        .then(|| picture_of(&preview.image.0))
        .flatten();
    Some(LinkInfo {
        uri: preview.uri.clone(),
        title: preview.title.clone(),
        description: preview.description.clone(),
        image,
    })
}

/// A text, or a text with its link's preview (`MCLink`).
#[must_use]
pub fn content_of(text: String, link: Option<&LinkInfo>) -> MsgContent {
    match link {
        Some(l) => MsgContent::Link {
            text,
            preview: to_chat(l),
        },
        None => MsgContent::Text { text },
    }
}
