use std::ops::Range;

use super::{Comment, Kind};

/// Read CMake line and bracket comments outside quoted and bracket arguments.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut unquoted = false;
    while let Some(&byte) = bytes.get(index) {
        let start = index;
        index += 1;
        match byte {
            b'#' => {
                if bytes.get(index) == Some(&b'[') {
                    let (after, level) = bracket_open(bytes, index);
                    index = after;
                    if let Some(level) = level {
                        let Some(end) = bracket_close(bytes, index, level) else {
                            break;
                        };
                        let body = text[index..end].trim_start_matches([' ', '\t']);
                        let body_start = end - body.len();
                        let body_end = body_start + body.trim_end_matches([' ', '\t']).len();
                        index = end + level + 2;
                        comments.push(Comment {
                            range: start..index,
                            kind: Kind::Contents(vec![Range {
                                start: body_start,
                                end: body_end,
                            }]),
                        });
                        unquoted = false;
                        continue;
                    }
                }
                index = match bytes[index..].iter().position(|byte| *byte == b'\n') {
                    Some(length) => {
                        let end = index + length;
                        end - usize::from(bytes[end - 1] == b'\r')
                    }
                    None => bytes.len(),
                };
                comments.push(Comment::line(start..index, 1));
                unquoted = false;
            }
            b'[' if !unquoted => {
                let (after, level) = bracket_open(bytes, start);
                index = after;
                if let Some(level) = level {
                    let Some(end) = bracket_close(bytes, index, level) else {
                        break;
                    };
                    index = end + level + 2;
                } else {
                    unquoted = true;
                }
            }
            b'"' => {
                let (after, legacy) = quoted_end(bytes, index);
                index = after;
                unquoted &= legacy;
            }
            b'\\'
                if index < bytes.len()
                    && !bytes[index..].starts_with(b"\r\n")
                    && bytes[index] != b'\n' =>
            {
                index += 1;
                unquoted = true;
            }
            b'$' if bytes.get(index) == Some(&b'(') => {
                let (after, closed) = make_variable_end(bytes, index + 1);
                unquoted = closed || after > index + 1;
                index = after;
            }
            b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' => unquoted = false,
            _ => unquoted = true,
        }
    }
    comments
}

/// The byte after the opener, or after its longest matching prefix.
fn bracket_open(bytes: &[u8], start: usize) -> (usize, Option<usize>) {
    let level = bytes[start + 1..]
        .iter()
        .take_while(|byte| **byte == b'=')
        .count();
    let at = start + 1 + level;
    if bytes.get(at) == Some(&b'[') {
        (at + 1, Some(level))
    } else {
        (at, None)
    }
}

/// The start of the first closing bracket with the opening level.
fn bracket_close(bytes: &[u8], mut at: usize, level: usize) -> Option<usize> {
    while let Some(length) = bytes[at..].iter().position(|byte| *byte == b']') {
        let start = at + length;
        let equals = bytes[start + 1..]
            .iter()
            .take_while(|byte| **byte == b'=')
            .count();
        at = start + 1 + equals;
        if equals == level && bytes.get(at) == Some(&b']') {
            return Some(start);
        }
    }
    None
}

/// The end of a quoted argument and whether it can belong to a legacy
/// unquoted argument.
fn quoted_end(bytes: &[u8], mut at: usize) -> (usize, bool) {
    let mut legacy = true;
    while let Some(&byte) = bytes.get(at) {
        at += 1;
        match byte {
            b'"' => break,
            b'\\' if at < bytes.len() => {
                legacy &= bytes[at] != b'\n' && !bytes[at..].starts_with(b"\r\n");
                at += 1;
            }
            b'$' if legacy && bytes.get(at) == Some(&b'(') => {
                (at, legacy) = make_variable_end(bytes, at + 1);
            }
            b'#' | b'(' | b')' | b'\n' | b'\r' => legacy = false,
            _ => {}
        }
    }
    (at, legacy)
}

/// Consume a legacy Make-style reference if it closes after its name.
/// Otherwise just consume the name after the opening parenthesis.
fn make_variable_end(bytes: &[u8], name: usize) -> (usize, bool) {
    let length = bytes[name..]
        .iter()
        .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
        .count();
    let end = name + length;
    let closed = bytes.get(end) == Some(&b')');
    (end + usize::from(closed), closed)
}
