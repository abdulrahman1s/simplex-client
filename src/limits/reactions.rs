//! Reactions: the emoji one may carry and how many one person gives an item (Protocol.hs `mrEmojiChar`, Internal.hs `maxMsgReactions`).

/// The reactions there are, in display form: `👍 👎 😀 😂 😢 ❤️ 🚀 ✅` (Protocol.hs
/// `mrEmojiChar`). Anything else is refused when sent and ignored when received.
pub const REACTIONS: &[&str] = &["👍", "👎", "😀", "😂", "😢", "❤️", "🚀", "✅"];

/// Reactions one person gives one item: 3 (Internal.hs `maxMsgReactions`). A fourth is refused
/// when sent and ignored when received.
pub const MAX_REACTIONS: usize = 3;

/// Whether `c` passes as a reaction on the wire. Upstream checks each `Char` against the
/// literal above, so U+FE0F, the selector inside ❤️, passes on its own too.
#[must_use]
pub fn is_reaction_char(c: char) -> bool {
    REACTIONS.iter().any(|r| r.contains(c))
}

/// A reaction as it travels: one of [`REACTIONS`], ❤ without its variation selector.
#[must_use]
pub fn wire_reaction(emoji: &str) -> Option<char> {
    let mut it = emoji.chars().filter(|&c| c != '\u{fe0f}');
    let c = it.next()?;
    (it.next().is_none() && REACTIONS.iter().any(|r| r.starts_with(c))).then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eight_reactions_travel_as_one_char_and_others_do_not() {
        let sent: Vec<char> = REACTIONS.iter().filter_map(|r| wire_reaction(r)).collect();
        assert_eq!(sent, ['👍', '👎', '😀', '😂', '😢', '❤', '🚀', '✅']);
        for other in ["🤡", "👍👍", "", "a", "\u{fe0f}", "❤\u{fe0f}\u{fe0f}x"] {
            assert_eq!(wire_reaction(other), None, "{other:?}");
        }
    }

    #[test]
    fn the_wire_set_is_upstreams_literal_char_by_char() {
        let upstream = "👍👎😀😂😢❤️🚀✅";
        assert!(upstream.chars().all(is_reaction_char));
        assert!(!is_reaction_char('🤡'));
    }
}
