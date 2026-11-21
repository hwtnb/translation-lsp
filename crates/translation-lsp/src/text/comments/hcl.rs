use rust_lexer::{is_id_continue, is_id_start};

use super::{Comment, Kind};

/// A template or the expression open inside one.
enum Frame<'a> {
    Quoted,
    Heredoc {
        marker: &'a str,
        at_line_start: bool,
    },
    Expression(usize),
}

/// Reads HCL comments, including those inside template expressions.
pub(super) fn parse(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut frames = Vec::new();
    let mut comments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let rest = &bytes[index..];
        match frames.last_mut() {
            Some(Frame::Heredoc {
                marker,
                at_line_start,
            }) if *at_line_start => {
                *at_line_start = false;
                let (after, closed) = heredoc_line(text, index, marker);
                index = after;
                if closed {
                    frames.pop();
                }
                continue;
            }
            Some(frame @ (Frame::Quoted | Frame::Heredoc { .. })) => {
                match (frame, byte) {
                    (Frame::Quoted, b'"') => {
                        frames.pop();
                    }
                    (Frame::Quoted, b'\\') if rest.len() > 1 => {
                        index += 2;
                        continue;
                    }
                    (Frame::Heredoc { at_line_start, .. }, b'\n') => {
                        *at_line_start = true;
                    }
                    _ if rest.starts_with(b"$${") || rest.starts_with(b"%%{") => {
                        index += 3;
                        continue;
                    }
                    _ if rest.starts_with(b"${") || rest.starts_with(b"%{") => {
                        frames.push(Frame::Expression(0));
                        index += 2;
                        continue;
                    }
                    _ => {}
                }
                index += 1;
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
            b'<' if rest.starts_with(b"<<") => {
                if let Some((after, marker)) = heredoc_opening(text, index + 2) {
                    frames.push(Frame::Heredoc {
                        marker,
                        at_line_start: true,
                    });
                    index = after;
                } else {
                    // The second '<' may start a heredoc, as in '<<<EOT'.
                    index += 1;
                }
            }
            b'#' | b'/' if byte == b'#' || rest.starts_with(b"//") => {
                let opening = if byte == b'#' { 1 } else { 2 };
                if let Some(length) = rest.iter().position(|&byte| byte == b'\n') {
                    index += length;
                    let end = index - usize::from(bytes[index - 1] == b'\r');
                    comments.push(Comment::line(start..end, opening));
                } else {
                    comments.push(Comment::line(start..bytes.len(), opening));
                    break;
                }
            }
            b'/' if rest.starts_with(b"/*") => {
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

/// Returns a heredoc's marker and the position just after its opening line.
fn heredoc_opening(text: &str, mut index: usize) -> Option<(usize, &str)> {
    if text.as_bytes().get(index) == Some(&b'-') {
        index += 1;
    }
    let start = index;
    let mut chars = text[index..].chars();
    let first = chars.next().filter(|&ch| is_id_start(ch))?;
    index += first.len_utf8();
    for ch in chars.take_while(|&ch| ch == '-' || is_id_continue(ch)) {
        index += ch.len_utf8();
    }
    let ending = if text[index..].starts_with("\r\n") {
        2
    } else if text[index..].starts_with('\n') {
        1
    } else {
        return None;
    };
    Some((index + ending, &text[start..index]))
}

/// Consumes the part of a line that could belong to a closing marker.
fn heredoc_line(text: &str, index: usize, marker: &str) -> (usize, bool) {
    let whitespace = |ch: char| ch != '\n' && ch.is_whitespace();
    let rest = text[index..].trim_start_matches(whitespace);
    let mut index = text.len() - rest.len();
    for (expected, found) in marker.chars().zip(rest.chars()) {
        if expected != found {
            return (index, false);
        }
        index += found.len_utf8();
    }
    let rest = text[index..].trim_start_matches(whitespace);
    let index = text.len() - rest.len();
    if rest.starts_with('\n') {
        (index + 1, true)
    } else {
        (index, false)
    }
}
