use super::{Comment, Kind};

/// What the reader is inside, kept on a stack of its own.
enum Frame {
    /// The code of a `#{…}` interpolation, with the `{` opened in it.
    Interpolation { braces: usize },
    /// A string, a charlist, a quoted atom, or the text of a sigil. It ends
    /// at `closing`, or for a heredoc at three of it that start a line after
    /// spaces and tabs.
    Text {
        closing: u8,
        heredoc: bool,
        interpolates: bool,
        sigil: bool,
    },
}

/// The doc whose text the frame at the bottom of the stack holds.
struct Doc {
    /// Where its opening delimiter, or the `~` of its sigil, is.
    start: usize,
    /// Where its text starts, after the opening delimiter.
    text: usize,
}

/// The operators of more than one character, in the order the tokenizer
/// tries them, so that a `~` that ends one starts no sigil.
const OPERATORS: [&[u8]; 39] = [
    b"=>", b"..//", b"~~~", b"...", b"===", b"!==", b"&&&", b"|||", b"^^^", b"+++", b"---", b"<<<",
    b">>>", b"~>>", b"<<~", b"<~>", b"<|>", b"<<", b">>", b"//", b"**", b"..", b"++", b"--", b"<>",
    b"|>", b"~>", b"<~", b"==", b"=~", b"!=", b"<=", b">=", b"&&", b"||", b"<-", b"\\\\", b"::",
    b"->",
];

/// Elixir's tokenizer: a comment runs from `#` to the end of its line, and
/// the code of a `#{…}` interpolation is read as code, comments included.
/// The string, heredoc, or sigil `~S` or `~s` given to `@moduledoc`, `@doc`,
/// or `@typedoc` is a docstring, unless it is unclosed; a comment in its
/// interpolations is not taken apart from it.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut stack = Vec::new();
    let mut doc: Option<Doc> = None;
    // A shebang line is no comment to translate.
    let mark = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut index = if bytes[mark..].starts_with(b"#!") {
        line_end(bytes, mark)
    } else {
        0
    };
    while index < bytes.len() {
        let start = index;
        if let Some(&Frame::Text {
            closing,
            heredoc,
            interpolates,
            sigil,
        }) = stack.last()
        {
            let mut closed = None;
            match bytes[index] {
                b'\n' if heredoc => {
                    index += 1;
                    index += spaces(bytes, index);
                    if bytes[index..].starts_with(&[closing; 3]) {
                        closed = Some(index);
                        index += 3;
                    }
                }
                byte if byte == closing && !heredoc => {
                    closed = Some(index);
                    index += 1;
                }
                b'\\' => {
                    index += 1;
                    // An escaped line break still starts a line where a
                    // heredoc can close.
                    if !bytes[index..].starts_with(b"\n") && !bytes[index..].starts_with(b"\r\n") {
                        index = (index + 1).min(bytes.len());
                    }
                }
                b'#' if interpolates && bytes.get(index + 1) == Some(&b'{') => {
                    index += 2;
                    stack.push(Frame::Interpolation { braces: 0 });
                }
                _ => index += 1,
            }
            if let Some(end) = closed {
                stack.pop();
                // A string before `:` and a blank is the key of a keyword.
                let key = closing == b'"'
                    && !heredoc
                    && !sigil
                    && bytes[index..].starts_with(b":")
                    && matches!(bytes.get(index + 1), Some(b' ' | b'\t' | b'\r' | b'\n'));
                if stack.is_empty()
                    && let Some(found) = doc.take()
                    && !key
                {
                    let contents = found.text..end;
                    comments.push(Comment {
                        range: found.start..index,
                        kind: Kind::Contents(vec![contents]),
                    });
                }
                if sigil {
                    index += bytes[index..]
                        .iter()
                        .take_while(|byte| byte.is_ascii_alphanumeric())
                        .count();
                }
            }
            continue;
        }
        match bytes[index] {
            b'#' => {
                index = line_end(bytes, index);
                if doc.is_none() {
                    comments.push(Comment::line(start..index, 1));
                }
            }
            b'"' | b'\'' => {
                let (frame, _, next) = open(bytes, index, true, false);
                stack.push(frame);
                index = next;
            }
            b'~' if bytes.get(index + 1).is_some_and(u8::is_ascii_alphabetic) => {
                match sigil(bytes, index) {
                    Ok((frame, _, next)) => {
                        stack.push(frame);
                        index = next;
                    }
                    Err(next) => index = next,
                }
            }
            // `:::`, `:@`, and `:.` are atoms, so the `@` starts no attribute
            // and the `.` calls nothing.
            b':' => match bytes.get(index + 1) {
                Some(b':') if bytes.get(index + 2) == Some(&b':') => index += 3,
                Some(b'@' | b':') => index += 2,
                Some(b'.') if bytes.get(index + 2) != Some(&b'.') => index += 2,
                // A quoted atom is never a heredoc.
                Some(&quote @ (b'"' | b'\'')) => {
                    stack.push(Frame::Text {
                        closing: quote,
                        heredoc: false,
                        interpolates: true,
                        sigil: false,
                    });
                    index += 2;
                }
                _ => index += 1,
            },
            // A character: `?` and one character, or `?\` and one.
            b'?' => {
                index += 1;
                if bytes.get(index) == Some(&b'\\') {
                    index += 1;
                }
                index += char_at(text, index).map_or(0, char::len_utf8);
            }
            // After a dot, and the blanks, line breaks, and comments after
            // it, a quote starts the quoted name of a function, and `@` is
            // the name of one.
            b'.' if bytes.get(index + 1) != Some(&b'.') => {
                index = blank_lines_end(bytes, index + 1, &mut comments, doc.is_some());
                if let Some(&quote @ (b'"' | b'\'')) = bytes.get(index) {
                    stack.push(Frame::Text {
                        closing: quote,
                        heredoc: false,
                        interpolates: true,
                        sigil: false,
                    });
                    index += 1;
                } else if bytes.get(index) == Some(&b'@') {
                    index += 1;
                }
            }
            b'0'..=b'9' => index = number_end(bytes, index),
            b'@' if stack.is_empty() => {
                let (next, found) = doc_opening(text, index, &mut comments);
                index = next;
                if found {
                    let opened = if bytes[index] == b'~' {
                        sigil(bytes, index).ok()
                    } else {
                        Some(open(bytes, index, true, false))
                    };
                    if let Some((frame, text, next)) = opened {
                        doc = Some(Doc { start: index, text });
                        stack.push(frame);
                        index = next;
                    }
                }
            }
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
            byte if byte.is_ascii_alphabetic() || byte == b'_' => index = name_end(text, index),
            byte if byte >= 0x80 => match char_at(text, index) {
                Some(character) if character.is_alphabetic() => index = name_end(text, index),
                Some(character) => index += character.len_utf8(),
                None => index += 1,
            },
            byte if byte.is_ascii_punctuation() => {
                index += OPERATORS
                    .iter()
                    .find(|operator| operator[0] == byte && bytes[index..].starts_with(operator))
                    .map_or(1, |operator| operator.len());
            }
            _ => index += 1,
        }
    }
    comments
}

/// Where the line that goes on at `at` ends, before CR or LF.
fn line_end(bytes: &[u8], at: usize) -> usize {
    bytes[at..]
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map_or(bytes.len(), |length| at + length)
}

/// How many spaces and tabs there are from `at`.
fn spaces(bytes: &[u8], at: usize) -> usize {
    bytes[at..]
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

/// The character at `at`, if a character starts there.
fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

/// Where the name that starts at `at` ends: ASCII letters and digits, `_`,
/// `@`, and characters other than ASCII that are no blank, then one `?` or
/// `!` if there is one.
fn name_end(text: &str, mut at: usize) -> usize {
    let bytes = text.as_bytes();
    loop {
        match bytes.get(at) {
            // `@` goes on in the name of an atom, as in `:foo@bar`, and the
            // tokenizer rejects it in other names.
            Some(&byte) if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'@') => at += 1,
            // Outside strings and comments, the tokenizer rejects a character
            // other than ASCII that cannot go on a name, so any but a blank
            // goes on.
            Some(&byte) if byte >= 0x80 => match char_at(text, at) {
                Some(character) if !character.is_whitespace() => at += character.len_utf8(),
                _ => break,
            },
            _ => break,
        }
    }
    if matches!(bytes.get(at), Some(b'?' | b'!')) {
        at += 1;
    }
    at
}

/// The frame of the text whose opening delimiter is at `at`, where its text
/// starts, and where reading goes on. `"""` and `'''` open a heredoc when
/// only spaces and tabs follow them on their line.
fn open(bytes: &[u8], at: usize, interpolates: bool, sigil: bool) -> (Frame, usize, usize) {
    let delimiter = bytes[at];
    if matches!(delimiter, b'"' | b'\'') && bytes[at..].starts_with(&[delimiter; 3]) {
        let after = at + 3 + spaces(bytes, at + 3);
        if bytes[after..].starts_with(b"\n") || bytes[after..].starts_with(b"\r\n") {
            let frame = Frame::Text {
                closing: delimiter,
                heredoc: true,
                interpolates,
                sigil,
            };
            return (frame, at + 3, after);
        }
    }
    let closing = match delimiter {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        other => other,
    };
    let frame = Frame::Text {
        closing,
        heredoc: false,
        interpolates,
        sigil,
    };
    (frame, at + 1, at + 1)
}

/// The sigil whose `~` at `at` is followed by a letter, opened as [`open`]
/// opens a text. A name of one lowercase letter interpolates; one of an
/// uppercase letter and more uppercase letters and digits does not. Where
/// the tokenizer rejects the name or finds no delimiter, the error is where
/// reading goes on.
fn sigil(bytes: &[u8], at: usize) -> Result<(Frame, usize, usize), usize> {
    let lowercase = bytes[at + 1].is_ascii_lowercase();
    let mut end = at + 2;
    if !lowercase {
        end += bytes[end..]
            .iter()
            .take_while(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            .count();
    }
    match bytes.get(end) {
        Some(byte) if byte.is_ascii_lowercase() => Err(at + 1),
        Some(&byte) if delimiter(byte) => Ok(open(bytes, end, lowercase, true)),
        _ => Err(end),
    }
}

/// Whether a sigil's text can start with `byte`.
fn delimiter(byte: u8) -> bool {
    matches!(byte, b'/' | b'|' | b'"' | b'\'' | b'(' | b'[' | b'{' | b'<')
}

/// After the `@` at `at`, where reading goes on, and whether the text of a
/// doc starts there: after the name `moduledoc`, `doc`, or `typedoc`, and an
/// optional `(`, a string or a sigil `~S` or `~s`. Spaces, tabs, and line
/// continuations may come between them, and after the `(` line breaks and
/// comments too, which go to `comments`.
fn doc_opening(text: &str, at: usize, comments: &mut Vec<Comment>) -> (usize, bool) {
    let bytes = text.as_bytes();
    let mut index = blank_end(bytes, at + 1, false);
    if !char_at(text, index).is_some_and(|character| character == '_' || character.is_alphabetic())
    {
        return (index, false);
    }
    let end = name_end(text, index);
    if !matches!(&text[index..end], "moduledoc" | "doc" | "typedoc") {
        return (end, false);
    }
    index = blank_end(bytes, end, false);
    if bytes.get(index) == Some(&b'(') {
        index = blank_lines_end(bytes, index + 1, comments, false);
    }
    let found = match &bytes[index..] {
        [b'"', ..] => true,
        [b'~', b's' | b'S', byte, ..] => delimiter(*byte),
        _ => false,
    };
    (index, found)
}

/// Where the blanks, line breaks, and comments from `at` end, as
/// [`blank_end`] reads them. The comments go to `comments`, unless they are
/// inside a doc.
fn blank_lines_end(bytes: &[u8], at: usize, comments: &mut Vec<Comment>, in_doc: bool) -> usize {
    let mut index = blank_end(bytes, at, true);
    while bytes.get(index) == Some(&b'#') {
        let end = line_end(bytes, index);
        if !in_doc {
            comments.push(Comment::line(index..end, 1));
        }
        index = blank_end(bytes, end, true);
    }
    index
}

/// Where the number that starts at `at` ends. After the digits of `0x`,
/// `0o`, or `0b`, a letter starts a name.
fn number_end(bytes: &[u8], at: usize) -> usize {
    let radix = match bytes.get(at..at + 2) {
        Some(b"0x") => 16,
        Some(b"0o") => 8,
        Some(b"0b") => 2,
        _ => 10,
    };
    if radix != 10 && bytes.get(at + 2).is_some_and(|&byte| digit(byte, radix)) {
        return digits_end(bytes, at + 2, radix);
    }
    let mut end = digits_end(bytes, at, 10);
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end = digits_end(bytes, end + 1, 10);
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        if matches!(bytes.get(end), Some(b'e' | b'E'))
            && bytes.get(end + 1 + sign).is_some_and(u8::is_ascii_digit)
        {
            end = digits_end(bytes, end + 1 + sign, 10);
        }
    }
    end
}

/// Where the digits from `at` end, with a `_` between two of them.
fn digits_end(bytes: &[u8], mut at: usize, radix: u32) -> usize {
    loop {
        match bytes.get(at) {
            Some(&byte) if digit(byte, radix) => at += 1,
            Some(b'_') if bytes.get(at + 1).is_some_and(|&byte| digit(byte, radix)) => at += 2,
            _ => return at,
        }
    }
}

/// Whether `byte` is a digit in `radix`.
fn digit(byte: u8, radix: u32) -> bool {
    char::from(byte).is_digit(radix)
}

/// Where the spaces, tabs, and line continuations from `at` end, and with
/// `lines` the line breaks too.
fn blank_end(bytes: &[u8], mut at: usize, lines: bool) -> usize {
    loop {
        match &bytes[at..] {
            [b' ' | b'\t', ..] => at += 1,
            [b'\\', b'\n', ..] => at += 2,
            [b'\\', b'\r', b'\n', ..] => at += 3,
            [b'\n', ..] if lines => at += 1,
            [b'\r', b'\n', ..] if lines => at += 2,
            _ => return at,
        }
    }
}
