use std::{mem, ops::Range};

use super::{Comment, Kind};

/// Which part of a documentation attribute can come next.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Start,
    Dash,
    Attribute,
    Parenthesis,
    Other,
}

/// Reads Erlang line comments and the first string of each documentation attribute.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut form = Form::Start;
    let mut index = if bytes.starts_with(b"#!") {
        line_end(bytes, 0)
    } else {
        0
    };

    while let Some(character) = text[index..].chars().next() {
        if is_blank(character) {
            index += character.len_utf8();
            continue;
        }
        if character == '%' {
            let start = index;
            let opening = bytes[start..]
                .iter()
                .take(usize::from(u8::MAX))
                .take_while(|&&byte| byte == b'%')
                .count() as u8;
            index = line_end(bytes, start + usize::from(opening));
            comments.push(Comment::line(start..index, opening));
            continue;
        }

        let previous = mem::replace(&mut form, Form::Other);
        let doc = matches!(previous, Form::Attribute | Form::Parenthesis);
        match character {
            '.' => {
                let width = bytes[index..]
                    .iter()
                    .take(3)
                    .take_while(|&&byte| byte == b'.')
                    .count();
                index += width;
                if width == 1 && text[index..].starts_with(|c| c == '%' || is_blank(c)) {
                    form = Form::Start;
                }
            }
            '-' if previous == Form::Start => {
                form = Form::Dash;
                index += 1;
            }
            'd' | 'm' if previous == Form::Dash => {
                let end = name_end(text, index);
                if matches!(&text[index..end], "doc" | "moduledoc") {
                    form = Form::Attribute;
                }
                index = end;
            }
            '(' if previous == Form::Attribute => {
                form = Form::Parenthesis;
                index += 1;
            }
            '"' => index = string(text, index, b'"', None, doc, &mut comments),
            '\'' => {
                index = quoted_end(text, index + 1, b'\'', true).map_or(bytes.len(), |end| end + 1);
            }
            '$' => {
                index = if bytes.get(index + 1) == Some(&b'\\') {
                    escape_end(text, index + 1)
                } else {
                    character_end(text, index + 1)
                };
            }
            '~' => {
                let opening = name_end(text, index + 1);
                let name = &text[index + 1..opening];
                index = if let Some(closing) = sigil_closing(bytes.get(opening).copied()) {
                    let doc = doc && matches!(name, "" | "s" | "S" | "b" | "B");
                    string(text, opening, closing, Some(name), doc, &mut comments)
                } else {
                    opening
                };
            }
            _ => index += character.len_utf8(),
        }
    }

    comments
}

/// The end of a comment or shebang, before LF or CR LF.
fn line_end(bytes: &[u8], at: usize) -> usize {
    let Some(length) = bytes[at..].iter().position(|&byte| byte == b'\n') else {
        return bytes.len();
    };
    let lf = at + length;
    lf - usize::from(lf > 0 && bytes[lf - 1] == b'\r')
}

/// Whether Erlang's tokenizer treats this character as whitespace.
fn is_blank(character: char) -> bool {
    matches!(character, '\0'..=' ' | '\u{80}'..='\u{a0}')
}

/// The end of an Erlang name, which can include Latin-1 letters and `@`.
fn name_end(text: &str, at: usize) -> usize {
    let name = text[at..].trim_start_matches(|character: char| {
        character.is_ascii_alphanumeric()
            || matches!(character, '_' | '@' | 'À'..='Ö' | 'Ø'..='ö' | 'ø'..='ÿ')
    });
    text.len() - name.len()
}

/// The closing delimiter paired with a sigil's opening delimiter.
fn sigil_closing(opening: Option<u8>) -> Option<u8> {
    match opening? {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        b'<' => Some(b'>'),
        delimiter @ (b'/' | b'|' | b'#' | b'`' | b'\'' | b'"') => Some(delimiter),
        _ => None,
    }
}

/// Reads a string from its opening delimiter, recording the source body of a doc.
fn string(
    text: &str,
    opening: usize,
    closing: u8,
    sigil: Option<&str>,
    doc: bool,
    comments: &mut Vec<Comment>,
) -> usize {
    let bytes = text.as_bytes();
    let escapes = matches!(sigil, None | Some("" | "s" | "b"));
    let (body, end, width) = if bytes[opening..].starts_with(b"\"\"\"") {
        let width = bytes[opening..]
            .iter()
            .take_while(|&&byte| byte == b'"')
            .count();
        let body = opening + width;
        let after = line_indent_end(text, body);
        match bytes.get(after) {
            Some(b'\n') => (
                body,
                triple_end(text, after + 1, width, matches!(sigil, Some("s" | "b"))),
                width,
            ),
            None => return bytes.len(),
            Some(_) => {
                // With text after the quotes, read the run as empty strings
                // and, if it is odd, the opening of one more string.
                if doc {
                    push_doc(comments, opening..opening + 2, opening + 1..opening + 1);
                }
                return if width.is_multiple_of(2) {
                    after
                } else {
                    quoted_end(text, after, b'"', true).map_or(bytes.len(), |end| end + 1)
                };
            }
        }
    } else {
        let body = opening + 1;
        (body, quoted_end(text, body, closing, escapes), 1)
    };

    let Some(end) = end else {
        return bytes.len();
    };
    if doc {
        push_doc(comments, opening..end + width, body..end);
    }
    end + width
}

/// Records a doc with its delimiters in the source range and its literal body inside.
fn push_doc(comments: &mut Vec<Comment>, range: Range<usize>, body: Range<usize>) {
    comments.push(Comment {
        range,
        kind: Kind::Contents(vec![body]),
    });
}

/// The first unescaped closing delimiter at or after `at`.
fn quoted_end(text: &str, mut at: usize, closing: u8, escapes: bool) -> Option<usize> {
    let bytes = text.as_bytes();
    while let Some(&byte) = bytes.get(at) {
        if byte == closing {
            return Some(at);
        }
        at = if byte == b'\\' && escapes {
            escape_end(text, at)
        } else {
            at + 1
        };
    }
    None
}

/// The closing quotes of a multiline string start a line after optional whitespace.
fn triple_end(text: &str, mut at: usize, width: usize, escapes: bool) -> Option<usize> {
    let bytes = text.as_bytes();
    loop {
        at = line_indent_end(text, at);
        let quotes = bytes[at..]
            .iter()
            .take(width)
            .take_while(|&&byte| byte == b'"')
            .count();
        if quotes == width {
            return Some(at);
        }
        at += quotes;

        loop {
            match bytes.get(at)? {
                b'\n' => {
                    at += 1;
                    break;
                }
                b'\\' if escapes => {
                    let newline = bytes.get(at + 1) == Some(&b'\n');
                    at = escape_end(text, at);
                    if newline {
                        break;
                    }
                }
                _ => at += 1,
            }
        }
    }
}

/// The end of a line's whitespace before its LF or first other character.
fn line_indent_end(text: &str, at: usize) -> usize {
    let rest = text[at..].trim_start_matches(|c| c != '\n' && is_blank(c));
    text.len() - rest.len()
}

/// The end of the escape that starts with the backslash at `at`.
fn escape_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    match &bytes[at + 1..] {
        [b'^', ..] => character_end(text, at + 2),
        [b'x', b'{', ..] => {
            let digits = bytes[at + 3..]
                .iter()
                .take_while(|byte| byte.is_ascii_hexdigit())
                .count();
            let end = at + 3 + digits;
            end + usize::from(bytes.get(end) == Some(&b'}'))
        }
        [b'x', high, low, ..] if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() => at + 4,
        [b'0'..=b'7', ..] => {
            at + 1
                + bytes[at + 1..]
                    .iter()
                    .take(3)
                    .take_while(|&&byte| matches!(byte, b'0'..=b'7'))
                    .count()
        }
        _ => character_end(text, at + 1),
    }
}

/// The end of the UTF-8 character at `at`, or `at` at EOF.
fn character_end(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}
