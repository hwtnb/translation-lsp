use super::{Comment, Kind};

/// The current part of an interpolated string.
enum Frame {
    /// Literal string text.
    String,
    /// Code in a hole, with one brace for the hole itself.
    Hole(usize),
}

/// Reads Lean 4 comments, including documentation and comments in interpolation holes.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut frames = Vec::new();
    let mut before_interp = 0;
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if matches!(frames.last(), Some(Frame::String)) {
            match byte {
                b'"' => {
                    index += 1;
                    frames.pop();
                }
                b'\\' => index = after_escape(text, index),
                b'{' => {
                    index += 1;
                    frames.push(Frame::Hole(1));
                }
                _ => index += 1,
            }
            continue;
        }

        let start = index;
        match &bytes[index..] {
            [b'-', b'-', ..] => {
                index = line_end(bytes, index + 2);
                comments.push(Comment::line(start..index, 2));
            }
            [b'/', b'-', ..] => {
                let Some(first) = char_at(text, index + 2) else {
                    break;
                };
                let opening = if matches!(first, '-' | '!') { 3 } else { 2 };
                // Lean reads one character after `/-` before counting nested delimiters.
                let Some(end) = block_end(bytes, index + 2 + first.len_utf8()) else {
                    break;
                };
                comments.push(Comment {
                    range: start..end,
                    kind: Kind::Block {
                        opening,
                        closed: true,
                    },
                });
                index = end;
            }
            [b'<' | b'*', b'.', b'.', b'.', b'<', ..] => index += 5,
            [b'.', b'.', b'.', b'<', ..] => index += 4,
            [b'<', b'<' | b'=', b'<', ..] | [b'=', b'<', b'<', ..] => index += 3,
            [b'<', b'-', ..] | [b'/', b'/' | b'\\', ..] | [b'\\', b'/', ..] => index += 2,
            [b'{', ..] => {
                if let Some(Frame::Hole(depth)) = frames.last_mut() {
                    *depth += 1;
                }
                index += 1;
            }
            [b'}', ..] => {
                if let Some(Frame::Hole(depth)) = frames.last_mut() {
                    *depth -= 1;
                    if *depth == 0 {
                        frames.pop();
                    }
                }
                index += 1;
            }
            [b'"', ..] => {
                let Some(end) = string_end(text, index + 1) else {
                    break;
                };
                index = end;
            }
            [b'\'', ..] => index = character_end(text, index),
            [b'`', ..] => {
                index += 1;
                if let Some(first) = char_at(text, index).filter(|&c| name_start(c)) {
                    index = name_end(text, index, first);
                }
            }
            [b'r', b'"' | b'#', ..] => {
                let hashes = hash_count(&bytes[index + 1..], usize::MAX);
                let quote = index + 1 + hashes;
                if bytes.get(quote) == Some(&b'"') {
                    let Some(end) = raw_string_end(bytes, quote + 1, hashes) else {
                        break;
                    };
                    index = end;
                } else {
                    index = quote;
                }
            }
            _ => {
                let Some(character) = char_at(text, index) else {
                    break;
                };
                if name_start(character) {
                    index = name_end(text, index, character);
                    if bytes[index - 1] == b'!' && bytes.get(index) == Some(&b'"') {
                        if frames.is_empty() {
                            before_interp = comments.len();
                        }
                        frames.push(Frame::String);
                        index += 1;
                    }
                } else {
                    index += character.len_utf8();
                }
            }
        }
    }
    // Comments inside an unclosed interpolation are hidden with the string.
    if !frames.is_empty() {
        comments.truncate(before_interp);
    }
    comments
}

/// The byte before LF, or before CR when LF is immediately preceded by CR.
fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |length| {
            let end = from + length;
            end - usize::from(bytes[end - 1] == b'\r')
        })
}

/// The end of the matching `-/`, scanning from the fourth character of the comment.
fn block_end(bytes: &[u8], mut index: usize) -> Option<usize> {
    let mut depth = 1;
    while index < bytes.len() {
        match &bytes[index..] {
            [b'-', b'/', ..] => {
                index += 2;
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            [b'/', b'-', ..] => {
                index += 2;
                depth += 1;
            }
            _ => index += 1,
        }
    }
    None
}

/// The end of a backslash and the character it escapes, or EOF.
fn after_escape(text: &str, at: usize) -> usize {
    at + 1 + char_at(text, at + 1).map_or(0, char::len_utf8)
}

/// The end of the string whose opening quote is before `index`.
fn string_end(text: &str, mut index: usize) -> Option<usize> {
    while let Some(&byte) = text.as_bytes().get(index) {
        match byte {
            b'"' => return Some(index + 1),
            b'\\' => index = after_escape(text, index),
            _ => index += 1,
        }
    }
    None
}

/// The number of consecutive hashes, up to the requested delimiter length.
fn hash_count(bytes: &[u8], limit: usize) -> usize {
    bytes
        .iter()
        .take(limit)
        .take_while(|&&byte| byte == b'#')
        .count()
}

/// The end of a raw string whose opening quote is before `index`.
fn raw_string_end(bytes: &[u8], mut index: usize, hashes: usize) -> Option<usize> {
    while let Some(offset) = bytes[index..].iter().position(|&byte| byte == b'"') {
        index += offset + 1;
        let found = hash_count(&bytes[index..], hashes);
        // Hashes after a failed closing quote cannot start another closing quote.
        index += found;
        if found == hashes {
            return Some(index);
        }
    }
    None
}

/// The end of a character literal or of the apostrophe alone if it is invalid.
fn character_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let body = at + 1;
    let end = match &bytes[body..] {
        [b'\\', b'\\' | b'"' | b'\'' | b'r' | b'n' | b't', ..] => body + 2,
        [b'\\', b'x', digits @ ..] if has_hex_digits(digits, 2) => body + 4,
        [b'\\', b'u', digits @ ..] if has_hex_digits(digits, 4) => body + 6,
        [b'\r', b'\n', ..] => body + 2,
        [b'\\' | b'\'', ..] => return body,
        _ => body + char_at(text, body).map_or(0, char::len_utf8),
    };
    if bytes.get(end) == Some(&b'\'') {
        end + 1
    } else {
        body
    }
}

/// Whether `bytes` starts with the given number of ASCII hex digits.
fn has_hex_digits(bytes: &[u8], count: usize) -> bool {
    bytes
        .get(..count)
        .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
}

/// The character at a byte offset, if it exists.
fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

/// Whether a character can start a name component.
fn name_start(character: char) -> bool {
    character.is_alphabetic() || matches!(character, '_' | '«')
}

/// The end of a name, including dotted components and guillemet-quoted components.
fn name_end(text: &str, mut index: usize, mut first: char) -> usize {
    loop {
        index += first.len_utf8();
        if first == '«' {
            let Some(end) = text[index..].find('»') else {
                return text.len();
            };
            index += end + '»'.len_utf8();
        } else {
            while let Some(character) = char_at(text, index) {
                if !character.is_alphanumeric() && !matches!(character, '_' | '\'' | '!' | '?') {
                    break;
                }
                index += character.len_utf8();
            }
        }
        if text.as_bytes().get(index) != Some(&b'.') {
            return index;
        }
        match char_at(text, index + 1) {
            Some(character) if name_start(character) => {
                index += 1;
                first = character;
            }
            _ => return index,
        }
    }
}
