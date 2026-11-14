use std::ops::{Range, RangeInclusive};

use super::{Comment, Kind, LineKind};

/// Haskell decides comments by tokens, as GHC reads them with no language
/// extension. Two or more `-` start a line comment, except inside an
/// operator: the dashes must not follow a symbol character, and the
/// character after them must not be one, so `-->` and `|--` are operators.
/// `{-` starts a block comment, which nests and runs to its `-}`, except in
/// `{-#`, which starts a pragma. Text inside strings and character
/// literals is not read; `'` after a name, as in `foldl'`, belongs to the
/// name, and `'` before one, as in `'name`, is a quote.
///
/// Every `-` at the start of a line comment is part of its mark, and so are
/// the Haddock marks `-- |` and `-- ^`, and `|` and `^` after `{-` or `{- `.
/// A line comment ends at LF only, as in GHC; the CR of a CRLF is left out
/// of it.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while let Some(character) = text[index..].chars().next() {
        let start = index;
        match character {
            // GHC reads the pragmas it knows, such as `RULES`, as code, so
            // what follows `{-#` is read on as code.
            '{' if bytes.get(index + 1) == Some(&b'-') && bytes.get(index + 2) != Some(&b'#') => {
                // An unclosed block comment hides the rest of the file.
                let Some(end) = nested_end(bytes, index + 2) else {
                    break;
                };
                index = end;
                comments.push(block(bytes, start..end));
            }
            '"' => index = string_end(bytes, index + 1),
            '\'' => index = quote_end(text, index),
            _ if character.is_ascii_digit() => index = number_end(bytes, index),
            _ if character.is_alphanumeric() || character == '_' => {
                index += text[index..]
                    .find(|character: char| !name(character))
                    .unwrap_or(text.len() - index);
            }
            _ if symbol(character) => {
                index += text[index..]
                    .find(|character: char| !symbol(character))
                    .unwrap_or(text.len() - index);
                let dashes = bytes[start..index]
                    .iter()
                    .take_while(|byte| **byte == b'-')
                    .count();
                // The dashes are the whole operator, so a character that is
                // no symbol, or the end of the text, follows them.
                if dashes >= 2 && start + dashes == index {
                    index = bytes[index..]
                        .iter()
                        .position(|byte| *byte == b'\n')
                        .map_or(bytes.len(), |length| index + length);
                    let end = if bytes[..index].ends_with(b"\r") {
                        index - 1
                    } else {
                        index
                    };
                    let haddock = dashes == 2
                        && matches!(bytes.get(start + 2..start + 4), Some(b" |" | b" ^"));
                    let kind = if haddock {
                        LineKind::Haddock
                    } else {
                        LineKind::Plain(u8::try_from(dashes).unwrap_or(u8::MAX))
                    };
                    comments.push(Comment {
                        range: start..end,
                        kind: Kind::Line(kind),
                    });
                }
            }
            _ => index += character.len_utf8(),
        }
    }
    comments
}

/// Whether GHC reads `character` as part of an operator. GHC takes symbols
/// and punctuation outside ASCII as symbol characters too, except brackets,
/// quotation marks, and some marks. The standard library has no Unicode
/// categories, so every character outside ASCII that is neither
/// alphanumeric nor whitespace counts as one here, except the brackets and
/// quotation marks of [`BRACKETS`].
fn symbol(character: char) -> bool {
    match character {
        '!' | '#' | '$' | '%' | '&' | '*' | '+' | '.' | '/' | '<' | '=' | '>' | '?' | '@'
        | '\\' | '^' | '|' | '-' | '~' | ':' => true,
        _ if character.is_ascii() => false,
        _ => {
            !character.is_alphanumeric()
                && !character.is_whitespace()
                && !BRACKETS.iter().any(|range| range.contains(&character))
        }
    }
}

/// Brackets and quotation marks that text commonly uses: those of Latin
/// scripts, of CJK, and their full-width forms.
const BRACKETS: [RangeInclusive<char>; 14] = [
    '\u{ab}'..='\u{ab}',
    '\u{bb}'..='\u{bb}',
    '\u{2018}'..='\u{201f}',
    '\u{2039}'..='\u{203a}',
    '\u{3008}'..='\u{3011}',
    '\u{3014}'..='\u{301b}',
    '\u{301d}'..='\u{301f}',
    '\u{ff08}'..='\u{ff09}',
    '\u{ff3b}'..='\u{ff3b}',
    '\u{ff3d}'..='\u{ff3d}',
    '\u{ff5b}'..='\u{ff5b}',
    '\u{ff5d}'..='\u{ff5d}',
    '\u{ff5f}'..='\u{ff60}',
    '\u{ff62}'..='\u{ff63}',
];

/// Whether `character` can continue a name.
fn name(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '\'')
}

/// The end of the number at `at`, as GHC reads one with no language
/// extension: `0x1F`, `0o17`, or digits with a fraction and an exponent, as
/// in `1.5e-3`. A `'` after it starts a character literal or a quote.
fn number_end(bytes: &[u8], at: usize) -> usize {
    let digits = |from: usize, digit: fn(&u8) -> bool| {
        from + bytes[from..].iter().take_while(|byte| digit(byte)).count()
    };
    let octal = |byte: &u8| matches!(byte, b'0'..=b'7');
    match bytes[at..] {
        [b'0', b'x' | b'X', next, ..] if next.is_ascii_hexdigit() => {
            return digits(at + 2, u8::is_ascii_hexdigit);
        }
        [b'0', b'o' | b'O', next, ..] if octal(&next) => return digits(at + 2, octal),
        _ => {}
    }
    let mut index = digits(at, u8::is_ascii_digit);
    if bytes.get(index) == Some(&b'.') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
        index = digits(index + 1, u8::is_ascii_digit);
    }
    let sign = usize::from(matches!(bytes.get(index + 1), Some(b'+' | b'-')));
    if matches!(bytes.get(index), Some(b'e' | b'E'))
        && bytes.get(index + 1 + sign).is_some_and(u8::is_ascii_digit)
    {
        index = digits(index + 1 + sign, u8::is_ascii_digit);
    }
    index
}

/// The end of a block comment whose `{-` is before `from`, after the `-}`
/// that closes it.
fn nested_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1_usize;
    let mut index = from;
    while index + 1 < bytes.len() {
        match &bytes[index..index + 2] {
            b"{-" => {
                depth += 1;
                index += 2;
            }
            b"-}" => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }
    None
}

/// A block comment over `range`, whose text leaves out its marks: `{-` with
/// a Haddock mark or the dashes after it, and `-}` with the dashes before it.
fn block(bytes: &[u8], range: Range<usize>) -> Comment {
    let inner = range.start + 2..range.end - 2;
    let text = &bytes[inner.clone()];
    let opening = match text {
        [b'|' | b'^', ..] => 1,
        [b' ', b'|' | b'^', ..] => 2,
        _ => text.iter().take_while(|byte| **byte == b'-').count(),
    };
    let closing = text[opening..]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'-')
        .count();
    let mut first = inner.start + opening;
    let last = inner.end - closing;
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

/// The end of a string whose opening quote is before `from`. A backslash
/// escapes the character after it; before blanks and line breaks, it starts a
/// gap, which runs to the next backslash. A line break outside a gap ends
/// the string, which GHC reports as an error, and so does the end of the
/// text.
fn string_end(bytes: &[u8], from: usize) -> usize {
    let blank =
        |byte: Option<&u8>| matches!(byte, Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c'));
    let mut index = from;
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return index + 1,
            b'\n' | b'\r' => return index,
            b'\\' if blank(bytes.get(index + 1)) => {
                index += 1;
                while blank(bytes.get(index)) {
                    index += 1;
                }
                index += usize::from(bytes.get(index) == Some(&b'\\'));
            }
            b'\\' => index += 2,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// The end of what starts with `'` at `at`: a character literal such as
/// `'a'`, `'"'`, `'\''`, or `'\NUL'`, or else the quote alone, as in `'name`
/// and `''Type`.
fn quote_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = at + 1;
    if bytes.get(index) == Some(&b'\\') {
        index += 1;
        // The escaped character, then the rest of a name or a number, as in
        // `'\NUL'` and `'\x7F'`.
        index += text[index..]
            .chars()
            .next()
            .filter(|character| !matches!(character, '\n' | '\r'))
            .map_or(0, char::len_utf8);
        while bytes.get(index).is_some_and(u8::is_ascii_alphanumeric) {
            index += 1;
        }
    } else {
        index += text[index..]
            .chars()
            .next()
            .filter(|character| !matches!(character, '\'' | '\n' | '\r'))
            .map_or(0, char::len_utf8);
    }
    if index > at + 1 && bytes.get(index) == Some(&b'\'') {
        index + 1
    } else {
        at + 1
    }
}
