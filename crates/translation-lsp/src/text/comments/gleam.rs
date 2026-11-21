use super::Comment;

/// Reads Gleam line, documentation, and module documentation comments.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            b'"' => {
                index += 1;
                while let Some(&byte) = bytes.get(index) {
                    index += 1;
                    if byte == b'"' {
                        break;
                    }
                    if byte == b'\\' && index < bytes.len() {
                        index += 1;
                    }
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let opening = match bytes.get(index + 2..index + 4) {
                    Some(b"//") => 4,
                    _ if bytes.get(index + 2) == Some(&b'/') => 3,
                    _ => 2,
                };
                index += usize::from(opening);
                while !matches!(bytes.get(index), None | Some(b'\r' | b'\n')) {
                    index += 1;
                }
                comments.push(Comment::line(start..index, opening));
            }
            _ => index += 1,
        }
    }
    comments
}
