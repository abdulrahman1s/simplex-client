//! The small subset of YAML that file descriptions use: block maps, block lists, plain or quoted scalars, `[]` and `{}`.
use super::yaml_line::{Line, is_dash, lines, split_key, unquote, value};
use super::yaml_node::Node;
use crate::limits::MAX_FILE_REPLICAS;

/// Nesting levels a description may use; it needs four (map, `replicas`, a server, `chunks`).
/// The parser recurses once per level, so this bounds its stack: a hostile line of dashes
/// would otherwise overflow it, which aborts the process.
const MAX_DEPTH: usize = 8;

/// Lines a description may have: a replica line each, a server line per replica at most, and the header.
const MAX_LINES: usize = 2 * MAX_FILE_REPLICAS + 32;

pub(super) fn parse(text: &str) -> Result<Node, String> {
    let mut lines = lines(text, MAX_LINES)?;
    let Some(first) = lines.first() else {
        return Ok(Node::Map(Vec::new()));
    };
    let (mut i, indent) = (0, first.indent);
    let node = block(&mut lines, &mut i, indent, 0)?;
    if i < lines.len() {
        return Err("unexpected indentation".into());
    }
    Ok(node)
}

/// The level below `depth`, refused before any recursion past `MAX_DEPTH`.
fn deeper(depth: usize) -> Result<usize, String> {
    if depth >= MAX_DEPTH {
        return Err("file description nested too deeply".into());
    }
    Ok(depth + 1)
}

fn block(lines: &mut [Line], i: &mut usize, indent: usize, depth: usize) -> Result<Node, String> {
    let depth = deeper(depth)?;
    if is_dash(&lines[*i].text) {
        return seq(lines, i, indent, depth);
    }
    if split_key(&lines[*i].text).is_some() {
        return map(lines, i, indent, depth);
    }
    let node = value(&lines[*i].text)?;
    *i += 1;
    Ok(node)
}

fn seq(lines: &mut [Line], i: &mut usize, indent: usize, depth: usize) -> Result<Node, String> {
    let mut items = Vec::new();
    while *i < lines.len() && lines[*i].indent == indent && is_dash(&lines[*i].text) {
        let rest = lines[*i].text[1..].trim_start().to_owned();
        if rest.is_empty() {
            *i += 1;
            items.push(nested(lines, i, indent, depth)?);
        } else {
            // "- a: b" is a map whose first line starts after the dash.
            let used = lines[*i].text.len() - rest.len();
            lines[*i] = Line {
                indent: indent + used,
                text: rest,
            };
            let inner = lines[*i].indent;
            items.push(block(lines, i, inner, depth)?);
        }
    }
    Ok(Node::Seq(items))
}

/// What follows a bare `key:` or `-`: a block indented deeper, else nothing.
fn nested(lines: &mut [Line], i: &mut usize, indent: usize, depth: usize) -> Result<Node, String> {
    match lines.get(*i).map(|l| l.indent) {
        Some(deeper) if deeper > indent => block(lines, i, deeper, depth),
        _ => Ok(Node::Scalar(String::new())),
    }
}

fn map(lines: &mut [Line], i: &mut usize, indent: usize, depth: usize) -> Result<Node, String> {
    let mut entries = Vec::new();
    while *i < lines.len() && lines[*i].indent == indent && !is_dash(&lines[*i].text) {
        let text = lines[*i].text.clone();
        let (key, rest) = split_key(&text).ok_or("expected key: value")?;
        *i += 1;
        let node = if !rest.is_empty() {
            value(rest)?
        } else if lines
            .get(*i)
            .is_some_and(|l| l.indent == indent && is_dash(&l.text))
        {
            seq(lines, i, indent, deeper(depth)?)?
        } else {
            nested(lines, i, indent, depth)?
        };
        entries.push((unquote(key)?, node));
    }
    Ok(Node::Map(entries))
}
