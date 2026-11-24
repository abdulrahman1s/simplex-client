//! The lines and scalars of a description's YAML: indentation, `key: value`, dashes and quoted strings.
use super::yaml_node::Node;

pub(super) struct Line {
    pub(super) indent: usize,
    pub(super) text: String,
}

pub(super) fn is_dash(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// `key: rest` or `key:`; a colon inside a scalar (`1:abc`) does not start a value.
pub(super) fn split_key(text: &str) -> Option<(&str, &str)> {
    if text.starts_with(['"', '\'']) {
        return None;
    }
    let at = text
        .find(": ")
        .or_else(|| text.ends_with(':').then(|| text.len() - 1))?;
    Some((text[..at].trim_end(), text[at + 1..].trim()))
}

pub(super) fn unquote(s: &str) -> Result<String, String> {
    let Some(inner) = s.strip_prefix('"').or_else(|| s.strip_prefix('\'')) else {
        return Ok(s.to_owned());
    };
    let quote = &s[..1];
    let inner = inner
        .strip_suffix(quote)
        .ok_or("unterminated quoted string")?;
    if quote == "'" {
        return Ok(inner.replace("''", "'"));
    }
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some(c @ ('"' | '\\' | '/')) => c,
            _ => return Err("unsupported escape in quoted string".into()),
        });
    }
    Ok(out)
}

pub(super) fn value(rest: &str) -> Result<Node, String> {
    match rest {
        "[]" => Ok(Node::Seq(Vec::new())),
        "{}" => Ok(Node::Map(Vec::new())),
        r if r.starts_with(['[', '{', '&', '*', '|', '>', '!']) => Err("unsupported YAML".into()),
        r => Ok(Node::Scalar(unquote(r)?)),
    }
}

/// The non-empty lines, refused past `max` of them.
pub(super) fn lines(text: &str, max: usize) -> Result<Vec<Line>, String> {
    let mut out = Vec::new();
    for raw in text.lines() {
        if raw.contains('\t') {
            return Err("tab in YAML".into());
        }
        let trimmed = raw.trim_end();
        let body = trimmed.trim_start();
        if body.is_empty() || body.starts_with('#') || body == "---" {
            continue;
        }
        if out.len() == max {
            return Err("too many lines in file description".into());
        }
        out.push(Line {
            indent: trimmed.len() - body.len(),
            text: body.to_owned(),
        });
    }
    Ok(out)
}
