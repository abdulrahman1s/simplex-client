//! A random display name for an incognito profile: an adjective and a different noun (ProfileGenerator.hs `generateRandomProfile`; its word lists, AGPL-3.0-only).

use crate::crypto::random_vec;

/// The upstream lists, adjectives then nouns, a blank line between.
const WORDS: &str = include_str!("profile_words.txt");

fn lists() -> (Vec<&'static str>, Vec<&'static str>) {
    let (adjectives, nouns) = WORDS.split_once("\n\n").unwrap_or((WORDS, ""));
    (adjectives.lines().collect(), nouns.lines().collect())
}

fn pick<'a>(words: &[&'a str]) -> Option<&'a str> {
    let bytes = random_vec(4).ok()?;
    let n = u32::from_le_bytes(bytes.try_into().ok()?);
    let i = usize::try_from(n).ok()? % words.len().max(1);
    words.get(i).copied()
}

/// `adjective <> noun`, the noun drawn again up to twice while it is the adjective.
#[must_use]
pub fn random_name() -> Option<String> {
    let (adjectives, nouns) = lists();
    let adjective = pick(&adjectives)?;
    let mut noun = pick(&nouns)?;
    for _ in 0..2 {
        if noun != adjective {
            break;
        }
        noun = pick(&nouns)?;
    }
    Some(format!("{adjective}{noun}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_random_name_is_one_of_the_adjectives_then_one_of_the_nouns() {
        let (adjectives, nouns) = lists();
        assert_eq!((adjectives.len(), nouns.len()), (2093, 1013));
        // One adjective can begin another ("Comic", "Comical"), so any split may be the one.
        let name = random_name().unwrap();
        let split = |a: &&str| name.strip_prefix(*a).is_some_and(|n| nouns.contains(&n));
        assert!(adjectives.iter().any(split), "{name}");
    }
}
