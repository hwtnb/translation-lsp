use super::{Comment, Kind};

/// The context to return to after the current string, path, or interpolation.
enum Frame {
    Quoted,
    Indented,
    Path,
    Expression(usize),
}

/// Reads Nix comments outside literals and inside their interpolations.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut frames = Vec::new();
    let mut comments = Vec::new();
    let mut uri_prefix_end = 0;
    let mut path_component_end = 0;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let rest = &bytes[index..];
        match frames.last_mut() {
            Some(Frame::Quoted | Frame::Indented | Frame::Path) if rest.starts_with(b"${") => {
                frames.push(Frame::Expression(0));
                index += 2;
                continue;
            }
            Some(Frame::Quoted) => {
                match byte {
                    b'"' => {
                        frames.pop();
                        index += 1;
                    }
                    b'\\' if rest.len() > 1 => index += 2,
                    b'$' if rest.starts_with(b"$$") => index += 2,
                    _ => index += 1,
                }
                continue;
            }
            Some(Frame::Indented) => {
                if rest.starts_with(b"'''") || rest.starts_with(b"''$") {
                    index += 3;
                } else if rest.starts_with(b"''\\") && rest.len() > 3 {
                    index += 4;
                } else if rest.starts_with(b"''") {
                    frames.pop();
                    index += 2;
                } else if rest.starts_with(b"$$") {
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            Some(Frame::Path) => {
                if path_char(byte) || (byte == b'/' && path_segment_starts(&rest[1..])) {
                    index += 1;
                } else {
                    frames.pop();
                }
                continue;
            }
            Some(Frame::Expression(depth)) => match byte {
                b'{' => *depth += 1,
                b'}' if *depth == 0 => {
                    frames.pop();
                }
                b'}' => *depth -= 1,
                _ => {}
            },
            None => {}
        }

        let start = index;
        match byte {
            b'"' => {
                frames.push(Frame::Quoted);
                index += 1;
            }
            b'\'' if rest.starts_with(b"''") => {
                frames.push(Frame::Indented);
                index += 2;
            }
            b'#' => {
                index = text[index..]
                    .find(['\r', '\n'])
                    .map_or(bytes.len(), |len| index + len);
                comments.push(Comment::line(start..index, 1));
            }
            b'/' if rest.starts_with(b"//") => index += 2,
            b'/' if rest.starts_with(b"/*") => {
                let Some(length) = text[index + 2..].find("*/") else {
                    break;
                };
                index += length + 4;
                let opening = match rest.get(2..) {
                    Some([b'*', next, ..]) if !matches!(next, b'/' | b'*') => 3,
                    _ => 2,
                };
                comments.push(Comment {
                    range: start..index,
                    kind: Kind::Block {
                        opening,
                        closed: true,
                    },
                });
            }
            _ => {
                if byte.is_ascii_alphabetic() {
                    let end = prefix_end(bytes, index, &mut uri_prefix_end, uri_scheme_char);
                    if let [b':', first, ..] = &bytes[end..]
                        && uri_char(*first)
                    {
                        index = end + 1;
                        index += bytes[index..]
                            .iter()
                            .take_while(|&&ch| uri_char(ch))
                            .count();
                        continue;
                    }
                }
                let end = if byte == b'~' {
                    index + 1
                } else {
                    prefix_end(bytes, index, &mut path_component_end, path_char)
                };
                if let [b'/', after @ ..] = &bytes[end..]
                    && path_segment_starts(after)
                {
                    frames.push(Frame::Path);
                    index = end + 1;
                } else if byte.is_ascii_alphabetic() || byte == b'_' {
                    index += rest.iter().take_while(|&&ch| identifier_char(ch)).count();
                } else {
                    index += 1;
                }
            }
        }
    }
    comments
}

/// Reuses the end of a failed literal prefix when an operator splits its run.
fn prefix_end(bytes: &[u8], index: usize, end: &mut usize, accepts: fn(u8) -> bool) -> usize {
    if index >= *end {
        *end = index
            + bytes[index..]
                .iter()
                .take_while(|&&byte| accepts(byte))
                .count();
    }
    *end
}

fn identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'' | b'-')
}

fn path_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
}

/// Whether a slash is followed by a path component or interpolation.
fn path_segment_starts(text: &[u8]) -> bool {
    text.first().is_some_and(|&byte| path_char(byte)) || text.starts_with(b"${")
}

fn uri_scheme_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

fn uri_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"%/?:@&=+$,-_.!~*'".contains(&byte)
}
