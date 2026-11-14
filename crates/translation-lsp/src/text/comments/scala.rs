use super::{Comment, Kind};

/// What the reader is inside, kept on a stack of its own.
enum Frame {
    /// The code of a `${…}` block, with the `{` opened in it.
    Block { braces: usize },
    /// An interpolated string, such as `s"…"` or `f"""…"""`.
    Text { multiline: bool },
}

/// The names that the lexer reads as keywords, which start no interpolated
/// string.
const KEYWORDS: &[&str] = &[
    "abstract",
    "case",
    "catch",
    "class",
    "def",
    "do",
    "else",
    "end",
    "enum",
    "export",
    "extends",
    "false",
    "final",
    "finally",
    "for",
    "forSome",
    "given",
    "if",
    "implicit",
    "import",
    "lazy",
    "macro",
    "match",
    "new",
    "null",
    "object",
    "override",
    "package",
    "private",
    "protected",
    "return",
    "sealed",
    "super",
    "then",
    "this",
    "throw",
    "trait",
    "true",
    "try",
    "type",
    "val",
    "var",
    "while",
    "with",
    "yield",
    "_",
];

/// Scala 3's lexer: block comments nest, and the code of a `${…}` block in an
/// interpolated string is read as code, comments included. XML literals are
/// not followed.
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
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if let Some(&Frame::Text { multiline }) = stack.last() {
            match bytes[index] {
                b'"' if multiline => {
                    let quotes = bytes[index..]
                        .iter()
                        .take_while(|&&byte| byte == b'"')
                        .count();
                    index += quotes;
                    // The last three of three or more quotes close the string.
                    if quotes >= 3 {
                        stack.pop();
                    }
                }
                b'"' => {
                    index += 1;
                    stack.pop();
                }
                // The line ends an unclosed string.
                b'\r' | b'\n' if !multiline => {
                    stack.pop();
                }
                b'\\' if !multiline => {
                    index += 1;
                    if matches!(bytes.get(index), Some(b'"' | b'\\')) {
                        index += 1;
                    }
                }
                // `$$` and `$"` stand for `$` and `"`.
                b'$' => match bytes.get(index + 1) {
                    Some(b'$' | b'"') => index += 2,
                    Some(b'{') => {
                        index += 2;
                        stack.push(Frame::Block { braces: 0 });
                    }
                    _ => index += 1,
                },
                _ => index += 1,
            }
            continue;
        }
        match bytes[index] {
            b'"' => index = string_end(bytes, index),
            b'\'' => index = quote_end(text, index),
            // A name between backticks, which ends at its line.
            b'`' => {
                index += 1;
                if let Some(length) = bytes[index..]
                    .iter()
                    .position(|byte| matches!(byte, b'`' | b'\r' | b'\n'))
                    .filter(|&length| length > 0 && bytes[index + length] == b'`')
                {
                    index += length + 1;
                }
            }
            b'0'..=b'9' => {
                index += bytes[index..]
                    .iter()
                    .take_while(|&&byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    .count();
            }
            b'{' => {
                if let Some(Frame::Block { braces }) = stack.last_mut() {
                    *braces += 1;
                }
                index += 1;
            }
            b'}' => {
                index += 1;
                match stack.last_mut() {
                    Some(Frame::Block { braces: 0 }) => {
                        stack.pop();
                    }
                    Some(Frame::Block { braces }) => *braces -= 1,
                    _ => {}
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = line_end(index);
                comments.push(Comment::line(start..index, 2));
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                // `/**/` is an empty comment, and `/**` opens a Scaladoc comment.
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
            _ => match char_at(text, index) {
                Some(character) if name_start(character) => {
                    // A leading `_` takes no operator characters: `_+` is
                    // two tokens.
                    index = name_end(text, index + character.len_utf8());
                    // A name right before a quote is an interpolator, unless
                    // it is a keyword.
                    if bytes.get(index) == Some(&b'"') && !KEYWORDS.contains(&&text[start..index]) {
                        let multiline = bytes[index..].starts_with(b"\"\"\"");
                        index += if multiline { 3 } else { 1 };
                        stack.push(Frame::Text { multiline });
                    }
                }
                Some(character) => index += character.len_utf8(),
                None => index += 1,
            },
        }
    }
    comments
}

/// The character at `at`, if a character starts there.
fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

/// Whether a name starts with `character`: a letter, `_`, or `$`.
fn name_start(character: char) -> bool {
    matches!(character, '_' | '$') || character.is_alphabetic()
}

/// Where a name that goes on at `at` ends: letters, digits, and `$`, and
/// after a `_` either more of them or operator characters.
fn name_end(text: &str, mut at: usize) -> usize {
    let bytes = text.as_bytes();
    loop {
        match bytes.get(at) {
            Some(b'_') => {
                at += 1;
                if bytes.get(at).is_some_and(|&byte| operator(byte)) {
                    return operator_end(bytes, at);
                }
            }
            Some(&byte) if byte.is_ascii_alphanumeric() || byte == b'$' => at += 1,
            Some(&byte) if byte >= 0x80 => match char_at(text, at) {
                Some(character) if character.is_alphanumeric() => at += character.len_utf8(),
                _ => return at,
            },
            _ => return at,
        }
    }
}

/// Whether `byte` is an ASCII character of an operator name.
fn operator(byte: u8) -> bool {
    matches!(
        byte,
        b'~' | b'!'
            | b'@'
            | b'#'
            | b'%'
            | b'^'
            | b'*'
            | b'+'
            | b'-'
            | b'<'
            | b'>'
            | b'?'
            | b':'
            | b'='
            | b'&'
            | b'|'
            | b'/'
            | b'\\'
    )
}

/// Where operator characters from `at` end, before a `//` or `/*`.
fn operator_end(bytes: &[u8], mut at: usize) -> usize {
    while let Some(&byte) = bytes.get(at) {
        if !operator(byte) || byte == b'/' && matches!(bytes.get(at + 1), Some(b'/' | b'*')) {
            break;
        }
        at += 1;
    }
    at
}

/// Where the string that opens at `at` ends. Only a string of three or more
/// quotes goes on past its line, and it has no escapes.
fn string_end(bytes: &[u8], mut at: usize) -> usize {
    if bytes[at..].starts_with(b"\"\"\"") {
        at += 3;
        while let Some(length) = bytes[at..].iter().position(|&byte| byte == b'"') {
            at += length;
            let quotes = bytes[at..].iter().take_while(|&&byte| byte == b'"').count();
            at += quotes;
            if quotes >= 3 {
                return at;
            }
        }
        return bytes.len();
    }
    at += 1;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'"' => return at + 1,
            b'\r' | b'\n' => break,
            b'\\' => {
                at += 1;
                if !matches!(bytes.get(at), None | Some(b'\r' | b'\n')) {
                    at += 1;
                }
            }
            _ => at += 1,
        }
    }
    at
}

/// Where the token that starts with the `'` at `at` ends: a character
/// literal, a quoted name such as `'name` or `'+`, or the `'` alone before
/// the `{` or `[` of a quote.
fn quote_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let Some(character) = char_at(text, at + 1) else {
        return at + 1;
    };
    let after = at + 1 + character.len_utf8();
    let closed = |end: usize| {
        if bytes.get(end) == Some(&b'\'') {
            end + 1
        } else {
            end
        }
    };
    if name_start(character) {
        if bytes.get(after) == Some(&b'\'') {
            after + 1
        } else {
            name_end(text, after)
        }
    } else if character.is_ascii() && operator(character as u8) && character != '\\' {
        if bytes.get(after) == Some(&b'\'') {
            after + 1
        } else {
            operator_end(bytes, after)
        }
    } else {
        match character {
            '{' | '[' | ' ' | '\t' if bytes.get(after) != Some(&b'\'') => at + 1,
            '\r' | '\n' => at + 1,
            '\\' => closed(escape_end(text, after)),
            _ => closed(after),
        }
    }
}

/// Where the escape whose letter is at `at`, after its `\`, ends.
fn escape_end(text: &str, mut at: usize) -> usize {
    let bytes = text.as_bytes();
    match bytes.get(at) {
        Some(b'u' | b'U') => {
            at += bytes[at..]
                .iter()
                .take_while(|&&byte| matches!(byte, b'u' | b'U'))
                .count();
            at + bytes[at..]
                .iter()
                .take(4)
                .take_while(|byte| byte.is_ascii_hexdigit())
                .count()
        }
        Some(b'\r' | b'\n') | None => at,
        Some(_) => at + char_at(text, at).map_or(1, char::len_utf8),
    }
}
