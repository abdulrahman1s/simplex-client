//! A profile as a group shows it: what direct messages and SimpleX links allow (Internal.hs `redactedMemberProfile`, `userProfileInGroup`).

use crate::agent::links::ConnectionLink;
use crate::chat::group::GroupMemberRole;
use crate::chat::profile::Profile;
use crate::engine::records::{GroupRecord, MemberRecord};

use super::role::{direct_messages_allowed, simplex_links_allowed};

/// `redactedMemberProfile g m`: `role` is the member's whose profile it is. Preferences never
/// go; the contact link only where links and direct messages are both allowed; the domain claim,
/// without its proof, only where direct messages are.
#[must_use]
pub(crate) fn redacted(g: &GroupRecord, role: &GroupMemberRole, p: &Profile) -> Profile {
    let direct = direct_messages_allowed(g, role);
    let links = simplex_links_allowed(g, role) && direct;
    Profile {
        short_descr: p
            .short_descr
            .as_deref()
            .and_then(|s| without_links(links, true, s)),
        description: p
            .description
            .as_deref()
            .and_then(|s| without_links(links, false, s)),
        contact_link: p.contact_link.clone().filter(|_| links),
        preferences: None,
        contact_domain: p
            .contact_domain
            .clone()
            .filter(|_| direct)
            .map(without_proof),
        ..p.clone()
    }
}

/// `processMemberProfileUpdate`: a member who is also a friend keeps the whole profile.
#[must_use]
pub(crate) fn member_profile(g: &GroupRecord, m: &MemberRecord, p: &Profile) -> Profile {
    if m.friend.is_some() {
        return p.clone();
    }
    redacted(g, &m.role, p)
}

fn without_proof(mut claim: serde_json::Value) -> serde_json::Value {
    if let Some(o) = claim.as_object_mut() {
        o.remove("proof");
    }
    claim
}

fn is_link(word: &str) -> bool {
    let w = word.trim_matches(|c: char| "()[]<>\"'.,;:!?*_~`".contains(c));
    ConnectionLink::parse(w).is_ok()
}

/// `removeSimplexLink`: a short description with a link goes whole, a long one loses the links.
fn without_links(allowed: bool, drop_on_link: bool, s: &str) -> Option<String> {
    if allowed || !s.split_whitespace().any(is_link) {
        return Some(s.to_owned());
    }
    if drop_on_link {
        return None;
    }
    let kept: Vec<&str> = s.split_inclusive(char::is_whitespace).collect();
    let kept: String = kept
        .into_iter()
        .filter(|w| !is_link(w.trim_end()))
        .collect();
    (!kept.trim().is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "https://smp4.simplex.im/a#lrdvu2d8A1GumSmoKb2krQmtKhWXq-tyGpHuM7aMwsw";

    #[test]
    fn a_short_description_with_a_link_is_dropped_where_links_are_not_allowed() {
        assert_eq!(without_links(false, true, &format!("me {LINK}")), None);
        assert_eq!(
            without_links(true, true, &format!("me {LINK}")),
            Some(format!("me {LINK}"))
        );
    }

    #[test]
    fn a_description_keeps_its_text_without_the_link() {
        assert_eq!(
            without_links(false, false, &format!("find me at {LINK} today")).as_deref(),
            Some("find me at today")
        );
        assert_eq!(without_links(false, false, LINK), None);
        assert_eq!(
            without_links(false, false, "no links").as_deref(),
            Some("no links")
        );
    }
}
