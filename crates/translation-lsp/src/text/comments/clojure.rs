use super::Comment;

/// Clojure decides comments by tokens alone, as its reader does: outside
/// strings, regular expressions `#"..."`, and character literals such as
/// `\;` and `\"`, `;` starts a comment that runs to the end of the line, and
/// every `;` at its start is part of its mark. A `;` inside a form that `#_`
/// discards, or inside `(comment ...)`, starts a comment too. `#!` also
/// starts a comment to the end of the line, which is skipped, as a shebang
/// is in other languages.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            b';' => {
                index = line_end(bytes, index);
                let marks = bytes[start..index]
                    .iter()
                    .take_while(|byte| **byte == b';')
                    .count();
                comments.push(Comment::line(
                    start..index,
                    u8::try_from(marks).unwrap_or(u8::MAX),
                ));
            }
            b'#' if bytes.get(index + 1) == Some(&b'!') => index = line_end(bytes, index),
            b'#' if bytes.get(index + 1) == Some(&b'"') => index = string_end(bytes, index + 2),
            b'"' => index = string_end(bytes, index + 1),
            // The character after a backslash belongs to a character literal,
            // also a quote, `;`, or a line break, and so does the rest of its
            // token, as in `\newline`.
            b'\\' => {
                index += 1 + text[index + 1..].chars().next().map_or(0, char::len_utf8);
                index = token_end(text, index);
            }
            b'#' | b'\'' => index += 1,
            _ => {
                let character = text[index..].chars().next().unwrap();
                index += character.len_utf8();
                // A symbol, keyword, or number, in which `#!` starts nothing,
                // as in `a#!b`. Clojure ends a number at `#`, so `1#!x`
                // starts a comment there; this reads it as one token.
                if !ends_token(character) {
                    index = token_end(text, index);
                }
            }
        }
    }
    comments
}

/// The end of a token whose characters continue at `from`, which `#`, `'`,
/// and `%` do not end.
fn token_end(text: &str, from: usize) -> usize {
    text[from..]
        .find(ends_token)
        .map_or(text.len(), |length| from + length)
}

/// Whether `character` is whitespace, a comma, or starts a form of its own,
/// as Clojure's reader decides where a token ends.
fn ends_token(character: char) -> bool {
    "\";@^`~()[]{}\\,".contains(character)
        || matches!(character, '\u{1c}'..='\u{1f}')
        || character.is_whitespace()
            && !matches!(character, '\u{85}' | '\u{a0}' | '\u{2007}' | '\u{202f}')
}

/// Clojure's reader ends a comment at CR as well as at LF.
fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|byte| matches!(byte, b'\n' | b'\r'))
        .map_or(bytes.len(), |length| from + length)
}

/// The end of a string or regular expression whose opening quote is before
/// `from`. A backslash escapes the character after it. An unclosed one hides
/// the rest of the file.
fn string_end(bytes: &[u8], from: usize) -> usize {
    let mut index = from;
    while let Some(&byte) = bytes.get(index) {
        index += 1;
        match byte {
            b'"' => return index,
            b'\\' => index += 1,
            _ => {}
        }
    }
    bytes.len()
}
