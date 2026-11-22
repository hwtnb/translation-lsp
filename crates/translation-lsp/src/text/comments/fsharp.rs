use super::{Comment, ocaml};

/// The style of an open interpolated string.
#[derive(Clone, Copy)]
enum InterpStyle {
    /// `$"…"`
    SingleQuote,
    /// `$@"…"` or `@$"…"`
    Verbatim,
    /// `$"""…"""`
    TripleQuote,
    /// `$$"""…"""` or more `$` signs; holds the number of leading `$` signs.
    Extended(usize),
}

/// What the reader is currently inside, kept on a stack of its own.
enum Frame {
    /// The literal text of an interpolated string.
    String(InterpStyle),
    /// The code inside an interpolation hole, with nested `{` counted in
    /// `braces` (starting at 1 for the hole) and remaining closing `}` tokens
    /// in `closing`.
    Hole { braces: usize, closing: usize },
}

/// Reads F# line comments (`//`, `///`, `////`) and nested block comments
/// (`(* … *)`), skipping strings, interpolated string text, character literals,
/// backtick identifiers, and comments inside `#!` on the first line.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut frames = Vec::new();
    // Comment count before the outermost open interpolated string, so an
    // unclosed interpolated string hides comments inside its holes too.
    let mut before_interp = 0;
    let mark = if text.starts_with('\u{feff}') { 3 } else { 0 };
    // A shebang line at the start of the file is not a comment to translate.
    let mut index = if bytes[mark..].starts_with(b"#!") {
        line_end(bytes, mark)
    } else {
        mark
    };
    while index < bytes.len() {
        let rest = &bytes[index..];
        if let Some(&Frame::String(style)) = frames.last() {
            match style {
                InterpStyle::SingleQuote => match bytes[index] {
                    b'"' => {
                        index = after_byte_suffix(bytes, index + 1);
                        frames.pop();
                    }
                    b'\\' => {
                        index += if matches!(bytes.get(index + 1), Some(b'\\' | b'"')) {
                            2
                        } else {
                            1
                        };
                    }
                    b'{' if bytes.get(index + 1) == Some(&b'{') => index += 2,
                    b'}' if bytes.get(index + 1) == Some(&b'}') => index += 2,
                    b'{' => {
                        index += 1;
                        frames.push(Frame::Hole {
                            braces: 1,
                            closing: 1,
                        });
                    }
                    _ => index += 1,
                },
                InterpStyle::Verbatim => match bytes[index] {
                    b'"' if bytes.get(index + 1) == Some(&b'"') => index += 2,
                    b'"' => {
                        index = after_byte_suffix(bytes, index + 1);
                        frames.pop();
                    }
                    b'{' if bytes.get(index + 1) == Some(&b'{') => index += 2,
                    b'}' if bytes.get(index + 1) == Some(&b'}') => index += 2,
                    b'{' => {
                        index += 1;
                        frames.push(Frame::Hole {
                            braces: 1,
                            closing: 1,
                        });
                    }
                    _ => index += 1,
                },
                InterpStyle::TripleQuote => match bytes[index] {
                    b'"' if rest.starts_with(b"\"\"\"") => {
                        index = after_byte_suffix(bytes, index + 3);
                        frames.pop();
                    }
                    b'{' if bytes.get(index + 1) == Some(&b'{') => index += 2,
                    b'}' if bytes.get(index + 1) == Some(&b'}') => index += 2,
                    b'{' => {
                        index += 1;
                        frames.push(Frame::Hole {
                            braces: 1,
                            closing: 1,
                        });
                    }
                    _ => index += 1,
                },
                InterpStyle::Extended(dollars) => match bytes[index] {
                    b'"' if rest.starts_with(b"\"\"\"") => {
                        index += 3;
                        frames.pop();
                    }
                    b'{' => {
                        let count = rest.iter().take_while(|&&byte| byte == b'{').count();
                        index += count;
                        if count >= dollars {
                            frames.push(Frame::Hole {
                                braces: 1,
                                closing: dollars,
                            });
                        }
                    }
                    b'}' => {
                        index += rest.iter().take_while(|&&byte| byte == b'}').count();
                    }
                    _ => index += 1,
                },
            }
            continue;
        }

        let start = index;
        match bytes[index] {
            b'(' if rest.starts_with(b"(*") => {
                if rest.starts_with(b"(*)") {
                    index += 3;
                } else {
                    let Some(end) = block_comment_end(text, index + 2) else {
                        break;
                    };
                    comments.push(ocaml::comment(bytes, start..end));
                    index = end;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let opening = match bytes.get(index + 2..index + 4) {
                    Some(b"//") => 4,
                    _ if bytes.get(index + 2) == Some(&b'/') => 3,
                    _ => 2,
                };
                index = line_end(bytes, index + usize::from(opening));
                comments.push(Comment::line(start..index, opening));
            }
            b'"' => {
                let end = if rest.starts_with(b"\"\"\"") {
                    triple_string_end(bytes, index + 3)
                } else {
                    single_string_end(bytes, index + 1)
                };
                let Some(end) = end else {
                    break;
                };
                index = after_byte_suffix(bytes, end);
            }
            b'\'' => index = quote_end(text, index, false),
            b'`' => index = backtick_end(bytes, index),
            // `[|` and `[<` are single tokens, so `|` or `<` does not merge with
            // a following operator, string prefix, or `//`.
            b'[' if matches!(bytes.get(index + 1), Some(b'|' | b'<')) => index += 2,
            b'{' => {
                if bytes.get(index + 1) == Some(&b'|') {
                    index += 2;
                } else {
                    if let Some(Frame::Hole { braces, .. }) = frames.last_mut() {
                        *braces += 1;
                    }
                    index += 1;
                }
            }
            b'}' => {
                index += 1;
                if let Some(Frame::Hole { braces, closing }) = frames.last_mut() {
                    if *braces > 1 {
                        *braces -= 1;
                    } else if *closing > 1 {
                        *closing -= 1;
                    } else {
                        frames.pop();
                    }
                }
            }
            b'#' => {
                if bytes.get(index + 1) == Some(&b':') {
                    index = line_end(bytes, index + 2);
                } else {
                    index += 1;
                }
            }
            // `=` directly before an interpolated string opener rewinds to `=` in F#.
            b'=' if starts_interpolated_string(&rest[1..]) => index += 1,
            b'$' => {
                let dollars = rest.iter().take_while(|&&byte| byte == b'$').count();
                if rest[dollars..].starts_with(b"\"\"\"") {
                    index += dollars + 3;
                    if frames.is_empty() {
                        before_interp = comments.len();
                    }
                    frames.push(Frame::String(if dollars == 1 {
                        InterpStyle::TripleQuote
                    } else {
                        InterpStyle::Extended(dollars)
                    }));
                } else if dollars == 1 && bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    if frames.is_empty() {
                        before_interp = comments.len();
                    }
                    frames.push(Frame::String(InterpStyle::SingleQuote));
                } else if dollars == 1 && rest[1..].starts_with(b"@\"") {
                    index += 3;
                    if frames.is_empty() {
                        before_interp = comments.len();
                    }
                    frames.push(Frame::String(InterpStyle::Verbatim));
                } else {
                    index = operator_end(bytes, index + dollars);
                }
            }
            b'@' => {
                if bytes.get(index + 1) == Some(&b'"') {
                    let Some(end) = verbatim_string_end(bytes, index + 2) else {
                        break;
                    };
                    index = after_byte_suffix(bytes, end);
                } else if rest[1..].starts_with(b"$\"") {
                    index += 3;
                    if frames.is_empty() {
                        before_interp = comments.len();
                    }
                    frames.push(Frame::String(InterpStyle::Verbatim));
                } else {
                    let end = operator_end(bytes, index + 1);
                    index = end
                        + usize::from(
                            matches!(&bytes[start..end], b"@>|" | b"@@>|")
                                && bytes.get(end) == Some(&b'}'),
                        );
                }
            }
            b':' => {
                index += if rest.starts_with(b":?>") {
                    3
                } else if matches!(bytes.get(index + 1), Some(b':' | b'>' | b'?' | b'=')) {
                    2
                } else {
                    1
                };
            }
            b'.' | b'?' => {
                let skip = rest
                    .iter()
                    .take_while(|&&byte| matches!(byte, b'.' | b'?'))
                    .count();
                if bytes
                    .get(index + skip)
                    .copied()
                    .is_some_and(is_core_op_char)
                {
                    index = operator_end(bytes, index + skip + 1);
                } else {
                    index += skip;
                }
            }
            byte if is_core_op_char(byte) => {
                let end = operator_end(bytes, index + 1);
                index = end
                    + usize::from(
                        matches!(&bytes[start..end], b"|" | b">|") && bytes.get(end) == Some(&b'}'),
                    );
            }
            b'0'..=b'9' => index = number_end(text, index),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                index = ident_end(text, index + 1);
                if matches!(bytes.get(index), Some(b'!' | b'#')) {
                    index += 1;
                }
            }
            byte if byte >= 0x80 => match char_at(text, index) {
                Some(character) if character.is_alphabetic() => {
                    index = ident_end(text, index + character.len_utf8());
                    if matches!(bytes.get(index), Some(b'!' | b'#')) {
                        index += 1;
                    }
                }
                Some(character) => index += character.len_utf8(),
                None => index += 1,
            },
            _ => index += 1,
        }
    }
    if !frames.is_empty() {
        comments.truncate(before_interp);
    }
    comments
}

/// The byte offset of the next `\r`, `\n`, or end of `bytes`.
fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map_or(bytes.len(), |length| from + length)
}

/// The end of a block comment whose opening `(*` is before `from`, after its
/// matching `*)`.
fn block_comment_end(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1_usize;
    let mut index = from;
    while index < bytes.len() {
        index = match &bytes[index..] {
            [b'(', b'*', b')', ..] => index + 3,
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
            [b'"', b'"', b'"', ..] => triple_string_end(bytes, index + 3)?,
            [b'"', ..] => single_string_end(bytes, index + 1)?,
            [b'@', b'"', ..] => verbatim_string_end(bytes, index + 2)?,
            [b'\'', ..] => quote_end(text, index, true),
            _ => index + 1,
        };
    }
    None
}

/// Advances past an optional byte-literal suffix `B`.
fn after_byte_suffix(bytes: &[u8], end: usize) -> usize {
    end + usize::from(bytes.get(end) == Some(&b'B'))
}

/// The end of a `"…"` string whose opening quote is before `from`.
fn single_string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Some(index + 1),
            b'\\' if matches!(bytes.get(index + 1), Some(b'\\' | b'"')) => index += 2,
            _ => index += 1,
        }
    }
    None
}

/// The end of an `@"…"` verbatim string whose opening quotes are before `from`.
fn verbatim_string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'"' {
            if bytes.get(index + 1) == Some(&b'"') {
                index += 2;
            } else {
                return Some(index + 1);
            }
        } else {
            index += 1;
        }
    }
    None
}

/// The end of a `"""…"""` triple-quoted string whose opening quotes are before
/// `from`.
fn triple_string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while let Some(offset) = bytes[index..].iter().position(|&byte| byte == b'"') {
        index += offset;
        if bytes[index..].starts_with(b"\"\"\"") {
            return Some(index + 3);
        }
        index += 1;
    }
    None
}

/// Whether `bytes` begins with an interpolated string opener (`$"`, `$+"""`,
/// `$@"`, or `@$"`).
fn starts_interpolated_string(bytes: &[u8]) -> bool {
    match bytes {
        [b'@', b'$', b'"', ..] => true,
        [b'$', b'"', ..] | [b'$', b'@', b'"', ..] => true,
        [b'$', ..] => {
            let dollars = bytes.iter().take_while(|&&byte| byte == b'$').count();
            bytes[dollars..].starts_with(b"\"\"\"")
        }
        _ => false,
    }
}

/// The end of what starts with `'` at `at`: a character literal or the `'`
/// alone. Inside a block comment, only single-character and simple escape
/// literals count.
fn quote_end(text: &str, at: usize, in_comment: bool) -> usize {
    let bytes = text.as_bytes();
    let hex = u8::is_ascii_hexdigit;
    let digit = u8::is_ascii_digit;
    let rest = &bytes[at + 1..];
    let end = match rest {
        [
            b'\\',
            b'\\' | b'"' | b'\'' | b'a' | b'f' | b'v' | b'n' | b't' | b'b' | b'r',
            b'\'',
            ..,
        ] => Some(at + 4),
        [b'\\', d1, d2, d3, b'\'', ..] if !in_comment && digit(d1) && digit(d2) && digit(d3) => {
            Some(at + 6)
        }
        [b'\\', b'x', h1, h2, b'\'', ..] if !in_comment && hex(h1) && hex(h2) => Some(at + 6),
        [b'\\', b'u', h1, h2, h3, h4, b'\'', ..]
            if !in_comment && hex(h1) && hex(h2) && hex(h3) && hex(h4) =>
        {
            Some(at + 8)
        }
        [b'\\', b'U', h1, h2, h3, h4, h5, h6, h7, h8, b'\'', ..]
            if !in_comment
                && [h1, h2, h3, h4, h5, h6, h7, h8]
                    .into_iter()
                    .all(u8::is_ascii_hexdigit) =>
        {
            Some(at + 12)
        }
        _ => match char_at(text, at + 1) {
            Some(character)
                if !matches!(character, '\\' | '\n' | '\r' | '\t' | '\x08')
                    && character.len_utf16() == 1
                    && bytes.get(at + 1 + character.len_utf8()) == Some(&b'\'') =>
            {
                Some(at + 1 + character.len_utf8() + 1)
            }
            _ => None,
        },
    };
    match end {
        Some(end) if !in_comment => after_byte_suffix(bytes, end),
        Some(end) => end,
        None => at + 1,
    }
}

/// The end of a backtick-quoted identifier or invalid backtick run starting at
/// `at`.
fn backtick_end(bytes: &[u8], at: usize) -> usize {
    let invalid_body = |byte: u8| matches!(byte, b'`' | b'\n' | b'\r' | b'\t');
    if !bytes[at..].starts_with(b"``") {
        return at + 1;
    }
    let mut index = at + 2;
    while let Some(&byte) = bytes.get(index) {
        if !invalid_body(byte) {
            index += 1;
        } else if byte == b'`'
            && bytes
                .get(index + 1)
                .is_some_and(|&next| !invalid_body(next))
        {
            index += 2;
        } else {
            break;
        }
    }
    if index > at + 2 {
        if bytes[index..].starts_with(b"``") {
            index + 2
        } else if bytes.get(index) == Some(&b'`') {
            index + 1
        } else {
            index
        }
    } else {
        at + bytes[at..]
            .iter()
            .take(4)
            .take_while(|&&byte| byte == b'`')
            .count()
    }
}

/// Whether `byte` is an F# `op_char`.
fn is_op_char(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'$'
            | b'%'
            | b'&'
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'/'
            | b'<'
            | b'='
            | b'>'
            | b'?'
            | b'@'
            | b'^'
            | b'|'
            | b'~'
            | b':'
    )
}

/// Whether `byte` is an `op_char` that can start a multi-character operator
/// after zero or more `.` or `?` characters.
fn is_core_op_char(byte: u8) -> bool {
    is_op_char(byte) && !matches!(byte, b'.' | b'?' | b':')
}

/// Advances past any trailing `op_char` bytes from `from`.
fn operator_end(bytes: &[u8], from: usize) -> usize {
    from + bytes[from..]
        .iter()
        .take_while(|&&byte| is_op_char(byte))
        .count()
}

/// The end of a numeric literal starting at `at`, including its suffix or
/// trailing identifier characters.
fn number_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|&&byte| byte == b'_' || byte.is_ascii_digit())
            .count()
    };
    let mut index = digits(at);
    if bytes[index..].starts_with(b"..") {
        return index + 2;
    }
    if bytes.get(index) == Some(&b'.') {
        index = digits(index + 1);
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(index + 1), Some(b'+' | b'-')));
        if bytes.get(index + 1 + sign).is_some_and(u8::is_ascii_digit) {
            index = digits(index + 1 + sign);
        }
    }
    ident_end(text, index)
}

/// The end of an identifier's continuation characters from `from`.
fn ident_end(text: &str, mut index: usize) -> usize {
    let bytes = text.as_bytes();
    while let Some(&byte) = bytes.get(index) {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'') {
            index += 1;
        } else if byte >= 0x80 {
            match char_at(text, index) {
                Some(character) if character.is_alphanumeric() => index += character.len_utf8(),
                _ => break,
            }
        } else {
            break;
        }
    }
    index
}

/// The character at `at`, if a character starts there.
fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}
