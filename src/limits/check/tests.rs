//! The stateless checks, one rule each.

use super::*;

fn code(r: &Request) -> Option<ErrorCode> {
    check(r).err().map(|e| e.code)
}

#[test]
fn a_reaction_outside_the_official_set_is_refused() {
    let react = |emoji: &str| Request::React {
        chat: ChatRef::Direct(1),
        item: 1,
        emoji: emoji.into(),
        on: true,
    };
    assert_eq!(code(&react("🤡")), Some(ErrorCode::ReactionNotAllowed));
    assert_eq!(code(&react("👍👍")), Some(ErrorCode::ReactionNotAllowed));
    assert_eq!(code(&react("❤️")), None);
    assert_eq!(code(&react("❤")), None);
}

#[test]
fn a_text_longer_than_a_whole_message_is_refused() {
    let send = |text: String| Request::Send {
        chat: ChatRef::Direct(1),
        text,
        reply_to: None,
        mentions: Vec::new(),
    };
    let long = "a".repeat(MAX_ENCODED_MSG_LENGTH + 1);
    assert_eq!(code(&send(long.clone())), Some(ErrorCode::TextTooLong));
    assert_eq!(code(&send("hi".into())), None);
    let edit = Request::Edit {
        chat: ChatRef::Direct(1),
        item: 1,
        text: long,
    };
    assert_eq!(code(&edit), Some(ErrorCode::TextTooLong));
}

#[test]
fn more_than_three_mentions_in_a_group_are_refused() {
    let send = |n: i64| Request::Send {
        chat: ChatRef::Group(1),
        text: (1..=n).map(|i| format!("@m{i} ")).collect(),
        reply_to: None,
        mentions: (1..=n).map(|i| (format!("m{i}"), i)).collect(),
    };
    assert_eq!(code(&send(4)), Some(ErrorCode::TooManyMentions));
    assert_eq!(code(&send(3)), None);
}

#[test]
fn a_group_name_upstream_would_change_is_refused() {
    for bad in ["", "  ", "J.Doe", "#club", "a  b"] {
        assert_eq!(
            code(&Request::NewGroup(bad.into())),
            Some(ErrorCode::InvalidName),
            "{bad:?}"
        );
    }
    assert_eq!(code(&Request::NewGroup("Club 1".into())), None);
    assert_eq!(
        code(&Request::NewGroup(" Club ".into())),
        None,
        "trimmed first"
    );
}
