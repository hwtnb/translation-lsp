use super::{Comment, Kind};

/// Read GLSL comments on logical lines while keeping physical byte ranges.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut input = std::iter::from_fn(|| next_byte(bytes, &mut index)).peekable();
    let mut comments = Vec::new();
    while let Some((start, byte)) = input.next() {
        match byte {
            b'/' if bytes.get(start + 1) == Some(&b'/') => {
                input.next();
                let end = input
                    .find(|&(_, byte)| byte == b'\n')
                    .map_or(bytes.len(), |(at, _)| at);
                comments.push(Comment::line(start..end, 2));
            }
            b'/' if bytes.get(start + 1) == Some(&b'*') => {
                input.next();
                while let Some((_, byte)) = input.next() {
                    if byte == b'*'
                        && let Some((at, _)) = input.next_if(|&(_, byte)| byte == b'/')
                    {
                        comments.push(Comment {
                            range: start..at + 1,
                            kind: Kind::Block {
                                opening: 2,
                                closed: true,
                            },
                        });
                        break;
                    }
                }
            }
            b'"' => {
                while let Some((_, byte)) = input.next() {
                    match byte {
                        b'"' | b'\n' => break,
                        b'\\' if !matches!(input.peek(), Some((_, b'\n'))) => {
                            input.next();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    comments
}

/// The next byte after line splicing, with its source offset and newlines
/// normalized to LF.
fn next_byte(bytes: &[u8], index: &mut usize) -> Option<(usize, u8)> {
    loop {
        let at = *index;
        let byte = *bytes.get(at)?;
        *index += 1;
        match byte {
            b'\\' if matches!(bytes.get(*index), Some(b'\r' | b'\n')) => {
                *index += if bytes[*index..].starts_with(b"\r\n") {
                    2
                } else {
                    1
                };
            }
            b'\r' => {
                *index += usize::from(bytes.get(*index) == Some(&b'\n'));
                return Some((at, b'\n'));
            }
            _ => return Some((at, byte)),
        }
    }
}
