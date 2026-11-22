use std::ops::Range;

use super::{Comment, Kind};

/// OCaml decides comments by tokens, as its lexer reads them: `(*` starts a
/// comment, also in `(*)`, and comments nest. Inside a comment, the lexer
/// reads strings, quoted strings such as `{id|...|id}`, and character
/// literals, so a `*)` inside one closes nothing and a `"` inside `'"'`
/// opens no string. Outside comments, text inside these literals is not
/// read. `'` after a name, as in `x'`, belongs to the name, and `'` before
/// one, as in the type variable `'a`, is a quote.
///
/// `(**` opens a documentation comment and `(***` an ordinary one. The text
/// of a comment leaves out the `*` after its `(` and before its `)`.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let end = if bytes[index..].starts_with(b"(*") {
            let end = comment_end(bytes, index + 2);
            if let Some(end) = end {
                comments.push(comment(bytes, start..end));
            }
            end
        } else {
            token_end(bytes, index)
        };
        // An unclosed comment or string hides the rest of the file.
        let Some(end) = end else {
            break;
        };
        index = end;
    }
    comments
}

/// The end of the token that starts at `at`, where no comment starts: a
/// string, a quoted string, a character literal or a quote, a name, or a
/// byte. `None` for a string that is not closed.
fn token_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes[at] {
        byte if name_start(byte) => Some(
            at + 1
                + bytes[at + 1..]
                    .iter()
                    .take_while(|byte| name_byte(**byte))
                    .count(),
        ),
        b'"' => string_end(bytes, at + 1),
        b'{' => match quoted_opening(bytes, at) {
            Some((delimiter, from)) => quoted_end(bytes, from, delimiter),
            None => Some(at + 1),
        },
        b'\'' => Some(quote_end(bytes, at)),
        _ => Some(at + 1),
    }
}

/// The end of a comment whose `(*` is before `from`, after the `*)` that
/// closes it.
fn comment_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1_usize;
    let mut index = from;
    while index < bytes.len() {
        index = match &bytes[index..] {
            [b'(', b'*', ..] => {
                depth += 1;
                index + 2
            }
            [b'*', b')', ..] => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 2);
                }
                index + 2
            }
            _ => token_end(bytes, index)?,
        };
    }
    None
}

/// A block comment over `range`, as used by OCaml and F#, whose text leaves
/// out its `(` and the `*` after it, its `)` and the `*` before it, and one
/// space after the opening.
pub(super) fn comment(bytes: &[u8], range: Range<usize>) -> Comment {
    let inner = &bytes[range.start + 1..range.end - 1];
    let opening = inner.iter().take_while(|byte| **byte == b'*').count();
    let closing = inner[opening..]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'*')
        .count();
    let mut first = range.start + 1 + opening;
    let last = range.end - 1 - closing;
    if first < last && bytes[first] == b' ' {
        first += 1;
    }
    Comment {
        range,
        kind: Kind::Contents(vec![Range {
            start: first,
            end: last,
        }]),
    }
}

fn name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0xc0
}

fn name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'' | 0x80..)
}

/// The end of a string whose opening quote is before `from`, after its
/// closing quote. A backslash escapes the byte after it.
fn string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Some(index + 1),
            b'\\' => index += 2,
            _ => index += 1,
        }
    }
    None
}

/// The delimiter of a quoted string that opens at `at`, as `{|`, `{id|`,
/// `{%ext|`, or `{%%ext id|` do, and where its text starts. A delimiter with
/// an uppercase ASCII letter opens none; every letter outside ASCII counts as
/// lowercase.
fn quoted_opening(bytes: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let mut index = at + 1;
    if bytes.get(index) == Some(&b'%') {
        index += 1 + usize::from(bytes.get(index + 1) == Some(&b'%'));
        // The name of the extension, such as `ext` or `ext.sub`.
        loop {
            if !bytes.get(index).copied().is_some_and(name_start) {
                return None;
            }
            index += 1 + bytes[index + 1..]
                .iter()
                .take_while(|byte| name_byte(**byte))
                .count();
            if bytes.get(index) != Some(&b'.') {
                break;
            }
            index += 1;
        }
        let blanks = bytes[index..]
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\x0c'))
            .count();
        if blanks == 0 {
            return (bytes.get(index) == Some(&b'|')).then_some((&[][..], index + 1));
        }
        index += blanks;
    }
    let start = index;
    index += bytes[index..]
        .iter()
        .take_while(|byte| matches!(byte, b'a'..=b'z' | b'_' | 0x80..))
        .count();
    (bytes.get(index) == Some(&b'|')).then_some((&bytes[start..index], index + 1))
}

/// The end of a quoted string whose text starts at `from`, after the `|`,
/// `delimiter`, and `}` that close it.
fn quoted_end(bytes: &[u8], from: usize, delimiter: &[u8]) -> Option<usize> {
    let mut index = from;
    while let Some(found) = bytes[index..].iter().position(|byte| *byte == b'|') {
        index += found + 1;
        let rest = &bytes[index..];
        if rest.starts_with(delimiter) && rest.get(delimiter.len()) == Some(&b'}') {
            return Some(index + delimiter.len() + 1);
        }
    }
    None
}

/// The end of what starts with `'` at `at`: a character literal such as
/// `'a'`, `'"'`, `'\''`, or `'\x41'`, `''`, or else the quote alone.
fn quote_end(bytes: &[u8], at: usize) -> usize {
    let hex = |byte: &u8| byte.is_ascii_hexdigit();
    let length = match &bytes[at + 1..] {
        [b'\'', ..] => 1,
        [
            b'\\',
            b'\\' | b'\'' | b'"' | b'n' | b't' | b'b' | b'r' | b' ',
            b'\'',
            ..,
        ] => 3,
        [b'\\', b'0'..=b'9', b'0'..=b'9', b'0'..=b'9', b'\'', ..] => 5,
        [
            b'\\',
            b'o',
            b'0'..=b'7',
            b'0'..=b'7',
            b'0'..=b'7',
            b'\'',
            ..,
        ] => 6,
        [b'\\', b'x', high, low, b'\'', ..] if hex(high) && hex(low) => 5,
        [byte, b'\'', ..] if !matches!(byte, b'\\' | b'\n' | b'\r') => 2,
        // A line break is `\r*\n`.
        rest => {
            let returns = rest.iter().take_while(|byte| **byte == b'\r').count();
            if rest[returns..].starts_with(b"\n'") {
                returns + 2
            } else {
                0
            }
        }
    };
    at + 1 + length
}
