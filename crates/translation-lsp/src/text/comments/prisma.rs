use super::{Comment, Kind};

/// Reads Prisma line and block comments outside double-quoted strings.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            b'"' => {
                let in_line = |at| !matches!(bytes.get(at), None | Some(b'\r' | b'\n'));
                index += 1;
                while in_line(index) {
                    let byte = bytes[index];
                    index += 1;
                    if byte == b'"' {
                        break;
                    }
                    // An escape does not carry an unfinished string across a line.
                    if byte == b'\\' && in_line(index) {
                        index += 1;
                    }
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let opening = if bytes.get(index + 2) == Some(&b'/') {
                    3
                } else {
                    2
                };
                index += usize::from(opening);
                while !matches!(bytes.get(index), None | Some(b'\r' | b'\n')) {
                    index += 1;
                }
                comments.push(Comment::line(start..index, opening));
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                // An unclosed block hides the rest of the document.
                let Some(length) = text[index + 2..].find("*/") else {
                    break;
                };
                index += length + 4;
                comments.push(Comment {
                    range: start..index,
                    kind: Kind::Block {
                        opening: 2,
                        closed: true,
                    },
                });
            }
            _ => index += 1,
        }
    }
    comments
}
