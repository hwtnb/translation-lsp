use super::{Comment, Kind};

/// What the reader is inside, kept on a stack of its own.
enum Frame {
    /// The code of a `${…}` interpolation, with the `{` opened in it.
    Interpolation { braces: usize },
    /// A string between `'` or `"`, or three of them, that is not raw.
    Text { quote: u8, multiline: bool },
}

/// Dart's scanner: block comments nest, and the code of a `${…}`
/// interpolation in a string is read as code, comments included.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let line_end = |at: usize| {
        bytes[at..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(bytes.len(), |length| at + length)
    };
    let mut comments = Vec::new();
    let mut stack = Vec::new();
    // A script tag on the first line is no comment to translate.
    let mark = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut index = if bytes[mark..].starts_with(b"#!") {
        line_end(mark)
    } else {
        0
    };
    while index < bytes.len() {
        let start = index;
        if let Some(&Frame::Text { quote, multiline }) = stack.last() {
            match bytes[index] {
                // The first three quotes close a string of three.
                byte if byte == quote => {
                    if !multiline {
                        index += 1;
                        stack.pop();
                    } else if bytes[index..].starts_with(&[quote; 3]) {
                        index += 3;
                        stack.pop();
                    } else {
                        index += 1;
                    }
                }
                // The line ends an unclosed string.
                b'\r' | b'\n' if !multiline => {
                    stack.pop();
                }
                b'\\' => {
                    index += 1;
                    if !matches!(bytes.get(index), None | Some(b'\r' | b'\n')) || multiline {
                        index = (index + 1).min(bytes.len());
                    }
                }
                b'$' if bytes.get(index + 1) == Some(&b'{') => {
                    index += 2;
                    stack.push(Frame::Interpolation { braces: 0 });
                }
                _ => index += 1,
            }
            continue;
        }
        match bytes[index] {
            quote @ (b'\'' | b'"') => {
                let multiline = bytes[index..].starts_with(&[quote; 3]);
                index += if multiline { 3 } else { 1 };
                stack.push(Frame::Text { quote, multiline });
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' => {
                index += bytes[index..]
                    .iter()
                    .take_while(|&&byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
                    })
                    .count();
                // `r` at the start of a token, before a quote, makes the
                // string raw: no escapes and no interpolations.
                if &bytes[start..index] == b"r"
                    && let Some(&quote @ (b'\'' | b'"')) = bytes.get(index)
                {
                    index = raw_end(bytes, index, quote);
                }
            }
            b'0'..=b'9' => index = number_end(bytes, index),
            b'{' => {
                if let Some(Frame::Interpolation { braces }) = stack.last_mut() {
                    *braces += 1;
                }
                index += 1;
            }
            b'}' => {
                index += 1;
                match stack.last_mut() {
                    Some(Frame::Interpolation { braces: 0 }) => {
                        stack.pop();
                    }
                    Some(Frame::Interpolation { braces }) => *braces -= 1,
                    _ => {}
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = line_end(index);
                let opening = if bytes[start..].starts_with(b"///") {
                    3
                } else {
                    2
                };
                comments.push(Comment::line(start..index, opening));
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                // `/**/` is an empty comment, and `/**` opens a documentation comment.
                let opening =
                    if bytes[index..].starts_with(b"/**") && !bytes[index..].starts_with(b"/**/") {
                        3
                    } else {
                        2
                    };
                index += 2;
                let mut depth = 0;
                let mut closed = false;
                while index < bytes.len() {
                    if bytes[index..].starts_with(b"/*") {
                        depth += 1;
                        index += 2;
                    } else if bytes[index..].starts_with(b"*/") {
                        index += 2;
                        if depth == 0 {
                            closed = true;
                            break;
                        }
                        depth -= 1;
                    } else {
                        index += 1;
                    }
                }
                comments.push(Comment {
                    range: start..index,
                    kind: Kind::Block { opening, closed },
                });
            }
            _ => index += 1,
        }
    }
    comments
}

/// Where the raw string whose first `quote` is at `at` ends. Only a string
/// of three quotes goes on past its line.
fn raw_end(bytes: &[u8], at: usize, quote: u8) -> usize {
    let closing = [quote; 3];
    if bytes[at..].starts_with(&closing) {
        let from = at + 3;
        return bytes[from..]
            .windows(3)
            .position(|window| window == closing)
            .map_or(bytes.len(), |length| from + length + 3);
    }
    let from = at + 1;
    match bytes[from..]
        .iter()
        .position(|&byte| byte == quote || matches!(byte, b'\r' | b'\n'))
    {
        Some(length) if bytes[from + length] == quote => from + length + 1,
        Some(length) => from + length,
        None => bytes.len(),
    }
}

/// Where the number that starts at `at` ends, so that a name or an `r`
/// right after it starts a token of its own.
fn number_end(bytes: &[u8], at: usize) -> usize {
    let digits = |at: usize, hex: bool| {
        at + bytes[at..]
            .iter()
            .take_while(|&&byte| {
                byte == b'_'
                    || if hex {
                        byte.is_ascii_hexdigit()
                    } else {
                        byte.is_ascii_digit()
                    }
            })
            .count()
    };
    let digit_at = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_digit);
    if bytes[at] == b'0'
        && matches!(bytes.get(at + 1), Some(b'x' | b'X'))
        && bytes.get(at + 2).is_some_and(u8::is_ascii_hexdigit)
    {
        return digits(at + 2, true);
    }
    let mut end = digits(at, false);
    if bytes.get(end) == Some(&b'.') && digit_at(end + 1) {
        end = digits(end + 1, false);
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        if digit_at(end + 1 + sign) {
            end = digits(end + 1 + sign, false);
        }
    }
    end
}
