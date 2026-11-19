use std::{borrow::Cow, time::Instant};

use super::{Comment, Kind, LineKind, syntax};

pub(super) fn parse(text: &str, deadline: Instant) -> Option<Vec<Comment>> {
    syntax::parse(
        &parser_source(text),
        tree_sitter_c_sharp::LANGUAGE.into(),
        syntax::Recovery::StopAt(opens_verbatim_string),
        deadline,
        |node| {
            if node.kind() != "comment" {
                return None;
            }
            let range = node.byte_range();
            let token = &text[range.clone()];
            let kind = if token.starts_with("//") {
                Kind::Line(LineKind::CSharp(if token.starts_with("///") {
                    3
                } else {
                    2
                }))
            } else {
                Kind::Block {
                    opening: if token.starts_with("/**") { 3 } else { 2 },
                    closed: token.ends_with("*/"),
                }
            };
            Some(Comment { range, kind })
        },
    )
}

/// Whether `fragment`, the source of a syntax error, opens a verbatim string
/// that it does not close. The grammar then reads the rest of the string,
/// which may run over lines, as code, with apparent comments.
fn opens_verbatim_string(fragment: &[u8]) -> bool {
    let mut rest = fragment;
    while let Some(at) = rest.iter().position(|&byte| byte == b'@') {
        rest = &rest[at + 1..];
        let opened = rest
            .strip_prefix(b"\"")
            .or_else(|| rest.strip_prefix(b"$\""));
        let Some(mut content) = opened else {
            continue;
        };
        loop {
            let Some(quote) = content.iter().position(|&byte| byte == b'"') else {
                return true;
            };
            content = &content[quote + 1..];
            // `""` is a quote inside the string.
            match content.strip_prefix(b"\"") {
                Some(after) => content = after,
                None => break,
            }
        }
        rest = content;
    }
    false
}

fn parser_source(text: &str) -> Cow<'_, [u8]> {
    let mut source = Cow::Borrowed(text.as_bytes());
    for (offset, character) in text.char_indices() {
        if matches!(character, '\u{85}' | '\u{2028}' | '\u{2029}') {
            // The grammar ends line comments only at CR/LF. Keep original byte
            // offsets while making the remaining C# line terminators visible.
            let bytes = source.to_mut();
            bytes[offset] = b'\n';
            bytes[offset + 1..offset + character.len_utf8()].fill(b' ');
        }
    }
    source
}
