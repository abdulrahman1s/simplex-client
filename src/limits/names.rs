//! Display names, bios and welcome messages (Commands.hs `mkValidName`, `checkValidName`; the apps' `MAX_BIO_LENGTH_BYTES`).

use unicode_properties::{GeneralCategory as G, GeneralCategoryGroup, UnicodeGeneralCategory};

/// A display name keeps at most 50 characters (Commands.hs `mkValidName`: `take 50`).
pub const MAX_NAME_CHARS: usize = 50;

/// The line about the user or a group: at most 160 bytes as JSON (apps `MAX_BIO_LENGTH_BYTES`,
/// measured by `chatJsonLength`, see [`json_len`](super::json_len)).
pub const MAX_BIO_BYTES: usize = 160;

/// A group's welcome message: at most 1 200 bytes as JSON (apps `maxByteCount` in the
/// welcome message view).
pub const MAX_WELCOME_BYTES: usize = 1200;

/// Characters a name never has.
const PROHIBITED: &str = ".,;/\\#@'\"`~";

/// Haskell's `isSpace`: the Latin-1 spaces, then the `Space` category.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t'..='\r' | '\u{a0}')
        || (u32::from(c) > 0x377 && c.general_category() == G::SpaceSeparator)
}

fn is_punct(c: char) -> bool {
    matches!(
        c.general_category(),
        G::ConnectorPunctuation | G::DashPunctuation | G::OtherPunctuation
    )
}

fn first_name_char(c: char) -> bool {
    let cat = c.general_category();
    c.general_category_group() == GeneralCategoryGroup::Letter
        || matches!(cat, G::DecimalNumber | G::OtherSymbol)
}

fn first_char(c: char) -> bool {
    first_name_char(c) || matches!(c.general_category(), G::CurrencySymbol | G::MathSymbol)
}

/// The name `name` becomes once its invalid characters go (Commands.hs `mkValidName`): no
/// leading punctuation, no `.,;/\#@'"`~`, one space between words, at most 50 characters,
/// and `name_1` written `name-1`.
#[must_use]
pub fn valid_name(name: &str) -> String {
    let (mut out, mut prev, mut punct) = (Vec::<char>::new(), '\0', 0u32);
    for c in name.chars() {
        let c = if is_space(c) { ' ' } else { c };
        let p = is_punct(c);
        let ok = if PROHIBITED.contains(c) {
            false
        } else if prev == '\0' {
            c > ' ' && first_name_char(c)
        } else if prev == ' ' {
            first_char(c) || (punct == 0 && p)
        } else if punct > 0 {
            first_char(c) || c == ' '
        } else {
            let mark = c.general_category_group() == GeneralCategoryGroup::Mark;
            first_char(c) || c == ' ' || mark || p
        };
        if ok {
            out.push(c);
            prev = c;
            punct = if p {
                punct + 1
            } else if c == ' ' {
                punct
            } else {
                0
            };
        }
    }
    out.truncate(MAX_NAME_CHARS);
    while out.last() == Some(&' ') {
        out.pop();
    }
    no_ldn_suffix(out.into_iter().collect())
}

/// `alice_1` → `alice-1`: the CLI's `_n` suffix is not part of a name.
fn no_ldn_suffix(s: String) -> String {
    let Some((rest, digits)) = s.rsplit_once('_') else {
        return s;
    };
    if digits.is_empty() || rest.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return s;
    }
    format!("{rest}-{digits}")
}

/// Whether `name` may be a display name as it is (Commands.hs `checkValidName`): not empty and
/// unchanged by [`valid_name`].
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && valid_name(name) == name
}

#[cfg(test)]
mod tests;
