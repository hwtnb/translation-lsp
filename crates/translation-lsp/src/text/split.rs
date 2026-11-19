//! Where a long text is cut into parts that are translated separately.

use std::{mem, ops::Range};

use pulldown_cmark::{Event, Tag};

use super::markdown::{parser, parsing_source};

/// The parts of `text`, each aiming at `chunk_bytes`: byte ranges in order
/// that cover the whole text without overlapping. A text of at most one and a
/// half times `chunk_bytes` is one part.
///
/// A Markdown text is cut only at the start of a block outside every other
/// block, and before a heading where it can be: sections, each from a heading
/// to the next, are grouped until the next would exceed `chunk_bytes`. Only a
/// section of more than twice `chunk_bytes` is cut between its own blocks, and
/// it shares a part with no other section but a small last one. A block
/// larger than `chunk_bytes` is never cut, nor is a heading parted from the
/// block after it, whatever their size. Plain text takes paragraphs,
/// separated by blank lines, in place of sections. A last part of less than
/// a quarter of `chunk_bytes` joins the part before it.
pub(crate) fn parts(text: &str, markdown: bool, chunk_bytes: usize) -> Vec<Range<usize>> {
    let limit = chunk_bytes.max(1);
    if text.len() <= limit.saturating_add(limit / 2) {
        return ranges(&[], text.len());
    }
    let blocks = if markdown {
        markdown_blocks(text)
    } else {
        paragraphs(text)
    };
    let mut cuts = Vec::new();
    let mut part_start = 0;
    let sections = sections(&blocks, text.len());
    for (section, inner) in sections {
        if section.end - section.start > limit.saturating_mul(2) {
            if section.start > part_start {
                cuts.push(section.start);
            }
            cuts.extend(group(section.clone(), inner, limit));
            if section.end < text.len() {
                cuts.push(section.end);
            }
            part_start = section.end;
        } else if section.end - part_start > limit && section.start > part_start {
            cuts.push(section.start);
            part_start = section.start;
        }
    }
    join_small_end(&mut cuts, text.len(), limit);
    ranges(&cuts, text.len())
}

/// A place where a part may start.
struct Block {
    start: usize,
    /// Whether a section starts there.
    section: bool,
}

/// The sections of a text, each with the starts of its blocks after its
/// first, from the places `blocks` where a part may start.
fn sections(blocks: &[Block], end: usize) -> Vec<(Range<usize>, &[Block])> {
    let mut sections = Vec::new();
    let mut start = 0;
    let mut first = 0;
    for (index, block) in blocks.iter().enumerate() {
        if block.section {
            sections.push((start..block.start, &blocks[first..index]));
            start = block.start;
            first = index + 1;
        }
    }
    sections.push((start..end, &blocks[first..]));
    sections
}

/// The starts of the parts after the first that the blocks of `section`
/// make, each block joining the part before it while that part stays within
/// `limit`.
fn group(section: Range<usize>, blocks: &[Block], limit: usize) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut part_start = section.start;
    let ends = blocks
        .iter()
        .map(|block| block.start)
        .chain([section.end])
        .skip(1);
    for (block, end) in blocks.iter().zip(ends) {
        if end - part_start > limit && block.start > part_start {
            cuts.push(block.start);
            part_start = block.start;
        }
    }
    join_small_end(&mut cuts, section.end, limit);
    cuts
}

/// Joins the last part to the one before it when it is smaller than a
/// quarter of `limit`.
fn join_small_end(cuts: &mut Vec<usize>, end: usize, limit: usize) {
    if cuts.last().is_some_and(|last| end - last < limit / 4) {
        cuts.pop();
    }
}

fn ranges(cuts: &[usize], end: usize) -> Vec<Range<usize>> {
    [0].iter()
        .chain(cuts)
        .zip(cuts.iter().chain([&end]))
        .map(|(&start, &end)| start..end)
        .collect()
}

/// The lines where a block outside every other block starts, after the
/// first line, with headings as the starts of sections. The block right
/// after a heading is left out, so that no part ends with a heading.
fn markdown_blocks(text: &str) -> Vec<Block> {
    let source = parsing_source(text);
    let mut blocks: Vec<Block> = Vec::new();
    let mut depth = 0usize;
    let mut previous_end = 0;
    let mut heading = false;
    for (event, range) in parser(&source).into_offset_iter() {
        let (start, section) = match event {
            Event::Start(tag) => {
                depth += 1;
                if depth > 1 {
                    continue;
                }
                (range.start, matches!(tag, Tag::Heading { .. }))
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    previous_end = range.end;
                }
                continue;
            }
            Event::Rule if depth == 0 => (range.start, false),
            _ => continue,
        };
        // A part never ends with a heading: the block after one, a heading
        // too, stays in the same part.
        let after_heading = mem::replace(&mut heading, section);
        // A block may be indented; the part starts with its line.
        if !after_heading
            && let Some(line) = line_start(text, start)
            && line > 0
            && line >= previous_end
            && blocks.last().is_none_or(|last| line > last.start)
        {
            blocks.push(Block {
                start: line,
                section,
            });
        }
        if depth == 0 {
            previous_end = range.end;
        }
    }
    blocks
}

/// The start of the line that `at` is on, when only spaces and tabs come
/// before `at` on that line.
fn line_start(text: &str, at: usize) -> Option<usize> {
    let before = &text.as_bytes()[..at];
    let indent = before
        .iter()
        .rev()
        .take_while(|&&byte| byte == b' ' || byte == b'\t')
        .count();
    let line = at - indent;
    (line == 0 || matches!(before[line - 1], b'\n' | b'\r')).then_some(line)
}

/// The starts of the paragraphs of plain text after the first: the lines
/// that follow a blank line and are not blank themselves.
fn paragraphs(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut after_blank = false;
    let mut start = 0;
    while start < text.len() {
        let rest = &text[start..];
        let length = rest.find(['\n', '\r']).unwrap_or(rest.len());
        let blank = rest[..length].trim().is_empty();
        if !blank && after_blank && start > 0 {
            blocks.push(Block {
                start,
                section: true,
            });
        }
        after_blank = blank;
        let ending = if rest[length..].starts_with("\r\n") {
            2
        } else {
            usize::from(length < rest.len())
        };
        start += length + ending;
    }
    blocks
}

#[cfg(test)]
mod tests;
