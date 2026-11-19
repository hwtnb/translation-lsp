use std::time::{Duration, Instant};

use super::*;

/// A paragraph of about `bytes` bytes, ending with a blank line.
fn paragraph(label: &str, bytes: usize) -> String {
    let mut text = format!("{label} ");
    while text.len() + 2 < bytes {
        text.push_str("word ");
    }
    text.truncate(bytes.saturating_sub(2).max(label.len()));
    text.push_str("\n\n");
    text
}

/// The parts of `text`, checking that they join into it.
fn split(text: &str, markdown: bool, limit: usize) -> Vec<&str> {
    let ranges = parts(text, markdown, limit);
    let mut joined = String::new();
    let mut end = 0;
    for range in &ranges {
        assert_eq!(range.start, end, "{ranges:?}");
        assert!(range.start < range.end || text.is_empty(), "{ranges:?}");
        end = range.end;
        joined.push_str(&text[range.clone()]);
    }
    assert_eq!(joined, text);
    ranges.into_iter().map(|range| &text[range]).collect()
}

#[test]
fn parts_join_into_the_text() {
    let mut markdown = String::new();
    for section in 0..6 {
        markdown.push_str(&format!("## Section {section}\n\n"));
        markdown.push_str(&paragraph("Text", 300));
        markdown.push_str("- item\n- item\n\n> quoted\n\n");
    }
    let plain = (0..12)
        .map(|index| paragraph(&format!("Paragraph {index}"), 200))
        .collect::<String>();
    for (text, markdown) in [(markdown, true), (plain, false)] {
        for text in [
            text.clone(),
            text.replace('\n', "\r\n"),
            text.replace('\n', "\r"),
            text.trim_end().to_owned(),
            format!("\n\n\n{text}\n\n\n"),
        ] {
            for limit in [1, 100, 512, 1024, 4096] {
                split(&text, markdown, limit);
            }
        }
    }
    assert_eq!(split("", true, 1), [""]);
    assert_eq!(split("", false, 1), [""]);
    assert_eq!(split("\n", true, 1), ["\n"]);
}

#[test]
fn a_text_of_at_most_one_and_a_half_parts_is_not_split() {
    let mut text = String::new();
    while text.len() < 1536 {
        text.push_str("# Heading\n\nText.\n\n");
    }
    text.truncate(1536);
    assert_eq!(split(&text, true, 1024).len(), 1);
    text.push('x');
    assert!(split(&text, true, 1024).len() > 1);
}

#[test]
fn markdown_is_cut_before_headings() {
    let mut text = String::new();
    for (section, level) in [(0, "#"), (1, "##"), (2, "###"), (3, "##"), (4, "######")] {
        text.push_str(&format!("{level} Section {section}\n\n"));
        text.push_str(&paragraph("Text", 600));
    }
    let parts = split(&text, true, 1024);
    assert_eq!(parts.len(), 5, "{parts:?}");
    for part in parts {
        assert!(
            part.trim_start_matches('#').starts_with(" Section"),
            "{part:?}"
        );
    }
    // A heading underlined with `=` or `-` and an indented one start sections too.
    let text = format!(
        "{}Setext\n======\n\n{}   ## Indented\n\n{}",
        paragraph("First", 900),
        paragraph("Second", 900),
        paragraph("Third", 900)
    );
    let parts = split(&text, true, 1024);
    assert_eq!(parts.len(), 3, "{parts:?}");
    assert!(parts[1].starts_with("Setext\n"));
    assert!(parts[2].starts_with("   ## Indented"));
}

#[test]
fn sections_are_grouped_until_the_next_exceeds_the_size() {
    let mut text = String::new();
    for section in 0..6 {
        text.push_str(&format!("# Section {section}\n\n"));
        text.push_str(&paragraph("Text", 300));
    }
    let parts = split(&text, true, 1024);
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(parts.iter().all(|part| part.len() <= 1024));
    assert!(parts[1].starts_with("# Section 3"));
}

#[test]
fn markdown_is_not_cut_inside_a_block() {
    let large = |opening: &str, line: &str, closing: &str| {
        let mut block = opening.to_owned();
        while block.len() < 3000 {
            block.push_str(line);
        }
        block.push_str(closing);
        block
    };
    for block in [
        large("```text\n", "# not a heading\n\nline\n", "```\n\n"),
        large("~~~\n", "## not a heading\n", "~~~\n\n"),
        large("    code\n", "    # not a heading\n\n", "\n"),
        large("| a | b |\n| --- | --- |\n", "| # cell | cell |\n", "\n"),
        large("", "- item\n\n  # heading in an item\n\n  text\n\n", "\n"),
        large("", "1. item\n\n   paragraph\n\n", "\n"),
        large("", "> quoted\n>\n> # heading in a quote\n>\n", "\n"),
        large("<div>\n", "# not markdown\ntext\n", "</div>\n\n"),
        large("<!--\n", "# comment\n\n", "-->\n\n"),
        large("[^note]: footnote\n", "    # footnote text\n\n", "\n"),
    ] {
        let text = format!(
            "# Before\n\n{}{block}{}# After\n\n{}",
            paragraph("Text", 400),
            paragraph("Text", 400),
            paragraph("Text", 400)
        );
        let start = text.find(&block).unwrap();
        let end = start + block.trim_end().len();
        for range in parts(&text, true, 1024) {
            assert!(
                range.start <= start || range.start >= end,
                "{block:?} cut at {}",
                range.start
            );
        }
    }
}

#[test]
fn a_section_of_more_than_twice_the_size_is_cut_between_its_blocks() {
    let mut text = "# Long\n\n".to_owned();
    for index in 0..10 {
        text.push_str(&paragraph(&format!("Paragraph {index}"), 400));
    }
    text.push_str("# Short\n\n");
    text.push_str(&paragraph("Text", 400));
    let parts = split(&text, true, 1024);
    assert!(parts.len() >= 4, "{parts:?}");
    assert!(parts[0].starts_with("# Long"));
    assert!(parts[1].starts_with("Paragraph"));
    assert!(parts.last().unwrap().starts_with("# Short"));
    for part in &parts[..parts.len() - 1] {
        assert!(part.len() <= 1024, "{part:?}");
    }
    // One of exactly twice the size stays whole.
    let mut text = "# Whole\n\n".to_owned();
    while text.len() < 2048 {
        text.push_str("Line of text.\n\n");
    }
    text.truncate(2048);
    let whole = text.len();
    text.push_str("# Next\n\n");
    text.push_str(&paragraph("Text", 600));
    let parts = split(&text, true, 1024);
    assert_eq!(parts[0].len(), whole, "{parts:?}");
}

#[test]
fn a_block_larger_than_the_size_is_not_cut() {
    let line = "word ".repeat(2000);
    for markdown in [true, false] {
        assert_eq!(split(&line, markdown, 1024), [line.as_str()]);
    }
    // The heading before it stays with it, and so do the headings before
    // that one.
    for headings in ["# Title\n\n", "# Title\n\n## Subtitle\n\n### Third\n"] {
        let text = format!("{headings}{line}\n\n{}", paragraph("End", 600));
        let parts = split(&text, true, 1024);
        assert_eq!(
            parts,
            [format!("{headings}{line}\n\n"), paragraph("End", 600)]
        );
    }
}

#[test]
fn no_part_ends_with_a_heading() {
    let mut text = String::new();
    for section in 0..8 {
        text.push_str(&format!("# Section {section}\n\n## Detail {section}\n\n"));
        text.push_str(&paragraph("Text", 700));
    }
    let parts = split(&text, true, 1024);
    assert!(parts.len() >= 4, "{parts:?}");
    for part in parts {
        let last = part.trim_end().lines().last().unwrap();
        assert!(!last.starts_with('#'), "{part:?}");
    }
}

#[test]
fn a_small_last_part_joins_the_one_before() {
    for (tail, count) in [(200, 2), (300, 3)] {
        let text = format!(
            "# A\n\n{}# B\n\n{}# C\n\n{}",
            paragraph("Text", 800),
            paragraph("Text", 800),
            paragraph("Text", tail)
        );
        assert_eq!(split(&text, true, 1024).len(), count, "{tail}");
    }
    // In a long section too.
    let mut text = "# Long\n\n".to_owned();
    for index in 0..5 {
        text.push_str(&paragraph(&format!("Paragraph {index}"), 900));
    }
    text.push_str("Last.\n");
    let parts = split(&text, true, 1024);
    assert_eq!(
        *parts.last().unwrap(),
        paragraph("Paragraph 4", 900) + "Last.\n"
    );
}

#[test]
fn plain_text_is_cut_between_paragraphs() {
    let text = format!(
        "{}{}   \n\t\n{}{}",
        paragraph("First", 700),
        paragraph("Second", 700),
        paragraph("Third", 700),
        paragraph("Fourth", 700)
    )
    .replace("\n\n", "\r\n\r\n");
    let parts = split(&text, false, 1024);
    assert_eq!(parts.len(), 4, "{parts:?}");
    assert!(parts[2].starts_with("Third"));
    // A line that would be a heading in Markdown starts no paragraph.
    let text = format!("Line\n# Not a section\n{}", "word ".repeat(400));
    assert_eq!(split(&text, false, 1024).len(), 1);
}

#[test]
fn splitting_finishes_repeated_pieces_in_seconds() {
    let size = 1 << 20;
    for piece in [
        "# Heading\n",
        "Paragraph.\n\n",
        "Line\n",
        "> ",
        ">\n",
        "> a\n",
        "- ",
        "- a\n",
        "  - a\n",
        "1. a\n",
        "```\n",
        "    code\n",
        "| a |\n",
        "<div>\n",
        "<!--\n",
        "[^a]: b\n",
        "---\n",
        "*",
        "[",
        "`",
        "\n",
        "\r",
        " ",
    ] {
        let text = piece.repeat(size / piece.len());
        for markdown in [true, false] {
            let started = Instant::now();
            parts(&text, markdown, 1024);
            // A debug build took at most 0.4 seconds, and 3.5 seconds at
            // background priority, which a slow runner may be like.
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "{piece:?} {markdown}"
            );
        }
    }
}

#[test]
fn the_long_sample_is_divided_inside_its_long_section() {
    let sample = include_str!("../../../../../samples/long-document.md").replace("\r\n", "\n");
    let parts = split(&sample, true, 1024);
    let starts: Vec<_> = parts
        .iter()
        .map(|part| part.lines().next().unwrap())
        .collect();
    assert_eq!(
        starts,
        [
            "# Connection pool",
            "## Closing connections",
            "A connection that the server closed is not used again, even when it looks idle. The pool finds out only when it sends the next request on it, so that request is sent once more on a new connection, and fails only if the new connection fails too.",
            "## Monitoring",
        ]
    );
}
