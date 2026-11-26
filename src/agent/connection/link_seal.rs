//! An address's link data made under a given root key: the fixed data signed and sealed, the user data signed and sealed (Agent.hs `prepareConnectionLink'`, `createConnectionForLink'`; Crypto/ShortLink.hs `encodeSignFixedData`).
use crate::crypto::{PrivateKey, random_vec};
use crate::encoding::VersionRange;

use super::address::contact_uri;
use super::queues::link_sender_id;
use crate::agent::error::Result;
use crate::agent::records::{NewLink, RcvQueue};
use crate::agent::short_link::{
    UserConnLinkData, UserContactData, contact_short_link_kdf, encode_sign_fixed_data,
    encode_sign_user_data, encrypt_fixed_data, encrypt_user_data,
};

/// NEW's link data, and the sealed fixed data again for later LSETs. `ucd` gets the link key,
/// for user data that names the link itself (a channel's profile does).
pub(super) fn sealed_link(
    rq: &RcvQueue,
    vr: VersionRange,
    root: &PrivateKey,
    entity_id: Vec<u8>,
    client_data: Option<String>,
    ucd: impl FnOnce(&[u8]) -> Result<UserContactData>,
) -> Result<(NewLink, Vec<u8>)> {
    let corr_id = random_vec(24)?;
    let req = contact_uri(rq, vr, link_sender_id(&corr_id), client_data)?;
    let (link_key, fixed) = encode_sign_fixed_data(root, vr, &req, Some(entity_id))?;
    let user = encode_sign_user_data(root, vr, UserConnLinkData::Contact(ucd(&link_key)?))?;
    let (_, key) = contact_short_link_kdf(&link_key)?;
    let fixed = encrypt_fixed_data(&key, &fixed)?;
    Ok((
        NewLink {
            corr_id,
            fixed: fixed.clone(),
            user: encrypt_user_data(&key, &user)?,
            link_key,
        },
        fixed,
    ))
}

/// New user data for an existing link, signed by its root key and sealed under its link key.
pub(super) fn sealed_user_data(
    vr: VersionRange,
    root: &PrivateKey,
    link_key: &[u8],
    ucd: UserContactData,
) -> Result<Vec<u8>> {
    let user = encode_sign_user_data(root, vr, UserConnLinkData::Contact(ucd))?;
    let (_, key) = contact_short_link_kdf(link_key)?;
    encrypt_user_data(&key, &user)
}
