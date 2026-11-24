//! A link's preview, sent with the message that has the link (Protocol.hs `LinkPreview`).

use serde::{Deserialize, Serialize};

use super::Picture;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkInfo {
    pub uri: String,
    pub title: String,
    pub description: String,
    /// A small JPEG or PNG of the page; none shows as SimpleX's 1x1 placeholder.
    #[serde(default)]
    pub image: Option<Picture>,
}
