use lsp_types::TextDocumentContentChangeEvent;

use super::*;
use crate::lsp::document::Document;

/// A comment line between two others, with a character of two UTF-16 units.
const TEXT: &str = "intro\n  // comment 🚀 here\nafter\n";
/// The comment of [`TEXT`].
const COMMENT: Range = range((1, 2), (1, 20));

const fn range(start: (u32, u32), end: (u32, u32)) -> Range {
    Range {
        start: Position {
            line: start.0,
            character: start.1,
        },
        end: Position {
            line: end.0,
            character: end.1,
        },
    }
}

fn edit(range: Range, text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: Some(range),
        range_length: None,
        text: text.into(),
    }
}

fn full(text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: text.into(),
    }
}

/// Version 1 of `text`, open in session 1.
fn document(text: &str) -> Document {
    Document::new(1, 1, "rust".into(), text.into())
}

/// Sends `changes` as the next version.
fn update(document: &mut Document, changes: Vec<TextDocumentContentChangeEvent>) {
    let version = document.version + 1;
    document.change(version, changes).unwrap();
}

fn text_at(document: &Document, range: Range) -> &str {
    let indexed = document.text.as_ref().unwrap();
    &indexed.as_str()[indexed.range(range).unwrap()]
}

/// Where [`COMMENT`] of version 1 of [`TEXT`] is after `updates`, checking
/// that it holds the same text there.
fn moved(updates: Vec<Vec<TextDocumentContentChangeEvent>>) -> Option<Range> {
    let mut document = document(TEXT);
    for changes in updates {
        update(&mut document, changes);
    }
    let found = document.current_range(1, 1, COMMENT)?;
    assert_eq!(text_at(&document, found), "// comment 🚀 here");
    Some(found)
}

#[test]
fn edits_before_a_range_move_it_and_edits_after_it_leave_it() {
    let insert = |at: (u32, u32), text: &str| vec![edit(range(at, at), text)];
    // A line added or removed before it.
    assert_eq!(
        moved(vec![insert((0, 0), "new\n")]),
        Some(range((2, 2), (2, 20)))
    );
    assert_eq!(
        moved(vec![vec![edit(range((0, 0), (1, 0)), "")]]),
        Some(range((0, 2), (0, 20)))
    );
    // Text before it on its line, counted in UTF-16 units.
    assert_eq!(
        moved(vec![insert((1, 0), "🚀前")]),
        Some(range((1, 5), (1, 23)))
    );
    // An insertion at its start moves it; one at its end leaves it.
    assert_eq!(
        moved(vec![insert((1, 2), "x")]),
        Some(range((1, 3), (1, 21)))
    );
    assert_eq!(moved(vec![insert((1, 20), "x")]), Some(COMMENT));
    assert_eq!(
        moved(vec![vec![edit(range((1, 20), (2, 5)), "\nlater")]]),
        Some(COMMENT)
    );
    // A replacement of several lines that ends on its line.
    assert_eq!(
        moved(vec![vec![edit(range((0, 2), (1, 1)), "a\nbc\n🚀")]]),
        Some(range((2, 3), (2, 21)))
    );
}

#[test]
fn an_edit_that_touches_a_range_leaves_it_nowhere() {
    for touched in [
        range((1, 1), (1, 3)),
        range((1, 5), (1, 6)),
        range((1, 19), (2, 1)),
        range((0, 3), (1, 4)),
        range((0, 0), (2, 0)),
        range((1, 2), (1, 20)),
        range((1, 8), (1, 8)),
    ] {
        assert_eq!(moved(vec![vec![edit(touched, "x")]]), None, "{touched:?}");
    }
}

#[test]
fn a_range_follows_the_edits_of_each_version_and_of_one_update_in_order() {
    let updates = || {
        vec![
            vec![edit(range((0, 0), (0, 0)), "first\n")],
            vec![edit(range((2, 0), (2, 0)), "🚀")],
            vec![edit(range((3, 0), (3, 5)), "later")],
            // The second change is in the text after the first.
            vec![
                edit(range((0, 0), (0, 0)), "x\n"),
                edit(range((3, 0), (3, 0)), "yy"),
            ],
        ]
    };
    assert_eq!(moved(updates()), Some(range((3, 6), (3, 24))));
    let mut document = document(TEXT);
    for changes in updates() {
        update(&mut document, changes);
    }
    for (version, old) in [
        (1, COMMENT),
        (2, range((2, 2), (2, 20))),
        (3, range((2, 4), (2, 22))),
        (4, range((2, 4), (2, 22))),
        (5, range((3, 6), (3, 24))),
    ] {
        assert_eq!(
            document.current_range(1, version, old),
            Some(range((3, 6), (3, 24))),
            "version {version}"
        );
    }
    // A version that never was, a newer one, and another session.
    assert_eq!(document.current_range(1, 0, COMMENT), None);
    assert_eq!(document.current_range(1, 6, COMMENT), None);
    assert_eq!(document.current_range(2, 1, COMMENT), None);
    assert_eq!(document.current_range(2, 5, COMMENT), None);
}

#[test]
fn a_replaced_or_lost_text_and_a_reopened_document_forget_older_versions() {
    let shift = || edit(range((0, 0), (0, 0)), "new\n");
    let mut replaced = document(TEXT);
    update(&mut replaced, vec![shift()]);
    update(&mut replaced, vec![full(TEXT)]);
    assert_eq!(replaced.current_range(1, 1, COMMENT), None);
    assert_eq!(replaced.current_range(1, 2, COMMENT), None);
    assert_eq!(replaced.current_range(1, 3, COMMENT), Some(COMMENT));
    // Edits after a full update in the same update.
    update(&mut replaced, vec![full(TEXT), shift()]);
    assert_eq!(replaced.current_range(1, 3, COMMENT), None);
    update(&mut replaced, vec![shift()]);
    assert_eq!(
        replaced.current_range(1, 4, range((2, 2), (2, 20))),
        Some(range((3, 2), (3, 20)))
    );

    // An edit outside the text, and a version that does not increase.
    for (version, changes) in [
        (3, vec![shift(), edit(range((9, 0), (9, 0)), "x")]),
        (2, vec![shift()]),
    ] {
        let mut lost = document(TEXT);
        update(&mut lost, vec![shift()]);
        assert!(lost.change(version, changes).is_err());
        assert_eq!(lost.current_range(1, 1, COMMENT), None);
        lost.change(4, vec![full(TEXT)]).unwrap();
        assert_eq!(lost.current_range(1, 1, COMMENT), None);
        assert_eq!(lost.current_range(1, 2, COMMENT), None);
        assert_eq!(lost.current_range(1, 4, COMMENT), Some(COMMENT));
    }

    let mut reopened = Document::new(2, 2, "rust".into(), TEXT.into());
    update(&mut reopened, vec![shift()]);
    assert_eq!(reopened.current_range(1, 2, COMMENT), None);
    assert_eq!(
        reopened.current_range(2, 2, COMMENT),
        Some(range((2, 2), (2, 20)))
    );
}

#[test]
fn versions_beyond_the_limits_are_forgotten() {
    let after = |line| edit(range((line, 0), (line, 0)), "x\n");
    let mut document = document(TEXT);
    for update_number in 0..MAX_UPDATES {
        update(&mut document, vec![after(2 + update_number as u32)]);
    }
    assert_eq!(document.current_range(1, 1, COMMENT), Some(COMMENT));
    update(&mut document, vec![after(0)]);
    assert_eq!(document.current_range(1, 1, COMMENT), None);
    let shifted = range((2, 2), (2, 20));
    assert_eq!(document.current_range(1, 2, COMMENT), Some(shifted));

    let mut document = self::document(TEXT);
    update(&mut document, vec![after(0)]);
    update(&mut document, (0..MAX_EDITS).map(|_| after(3)).collect());
    assert_eq!(document.current_range(1, 2, shifted), Some(shifted));
    update(&mut document, vec![after(3)]);
    assert_eq!(document.current_range(1, 2, COMMENT), None);
    assert_eq!(document.current_range(1, 3, shifted), Some(shifted));
    update(&mut document, (0..=MAX_EDITS).map(|_| after(3)).collect());
    assert_eq!(document.current_range(1, 4, shifted), None);
    assert_eq!(document.current_range(1, 5, shifted), Some(shifted));
}

#[test]
fn a_line_ending_joined_across_an_edit_stops_what_it_moved() {
    // An LF inserted after a CR moves nothing after it, but the end of the
    // CR's line is no longer where it was.
    let mut document = document("a\rb");
    update(&mut document, vec![edit(range((1, 0), (1, 0)), "\n")]);
    assert_eq!(text_at(&document, range((1, 0), (1, 1))), "b");
    assert_eq!(
        document.current_range(1, 1, range((1, 0), (1, 1))),
        Some(range((1, 0), (1, 1)))
    );
    assert_eq!(
        document.current_range(1, 1, range((0, 0), (0, 1))),
        Some(range((0, 0), (0, 1)))
    );
    assert_eq!(document.current_range(1, 1, range((0, 0), (1, 0))), None);

    // A CR inserted before an LF.
    let mut document = self::document("a\nb");
    update(&mut document, vec![edit(range((0, 1), (0, 1)), "x\r")]);
    assert_eq!(document.current_range(1, 1, range((1, 0), (1, 1))), None);
    assert_eq!(
        document.current_range(1, 1, range((0, 0), (0, 1))),
        Some(range((0, 0), (0, 1)))
    );
}

#[test]
fn ranges_that_no_text_has_are_not_followed() {
    let mut history = History::default();
    history.record(
        1,
        vec![Edit::new(
            range((0, 0), (0, 0)),
            Some(Position::new(0, 5)),
            false,
        )],
    );
    assert_eq!(history.map(1, range((0, 3), (0, 1))), None);
    assert_eq!(history.map(1, range((0, 0), (0, u32::MAX))), None);
    history.record(
        2,
        vec![Edit::new(
            range((0, 0), (0, 0)),
            Some(Position::new(1, 0)),
            false,
        )],
    );
    assert_eq!(history.map(2, range((u32::MAX, 0), (u32::MAX, 1))), None);
    assert_eq!(
        history.map(1, range((0, 1), (2, 0))),
        Some(range((1, 6), (3, 0)))
    );
}

/// Every edit of a short text, with every replacement, moves every range
/// that it does not touch by as many bytes as it adds before it, and leaves
/// out only the ranges it touches or a line ending that it joins.
#[test]
fn every_edit_of_short_texts_moves_ranges_as_their_bytes_move() {
    let mut texts = vec![String::new()];
    let mut level = vec![String::new()];
    for _ in 0..3 {
        level = level
            .iter()
            .flat_map(|prefix| ["a", "\r", "\n", "🚀"].map(|suffix| format!("{prefix}{suffix}")))
            .collect();
        texts.extend(level.clone());
    }
    let points = |document: &Document| {
        let indexed = document.text.as_ref().unwrap();
        (0..=indexed.as_str().len())
            .filter_map(|offset| Some((offset, indexed.position(offset).ok()?)))
            .collect::<Vec<_>>()
    };
    for text in texts {
        let positions = &points(&document(&text));
        let pairs = || {
            positions.iter().flat_map(move |&(start, from)| {
                positions
                    .iter()
                    .filter(move |&&(end, _)| start <= end)
                    .map(move |&(end, to)| (start..end, Range::new(from, to)))
            })
        };
        for (replaced, edited) in pairs() {
            for replacement in ["", "x", "\r", "\n", "\r\n", "🚀\n"] {
                let mut document = document(&text);
                update(&mut document, vec![edit(edited, replacement)]);
                let new = document.text.as_ref().unwrap().as_str().as_bytes();
                let inserted_end = replaced.start + replacement.len();
                let joined =
                    |at: usize| at > 0 && new[at - 1] == b'\r' && new.get(at) == Some(&b'\n');
                let joins = joined(replaced.start) || joined(inserted_end);
                for (bytes, range) in pairs() {
                    let expected = if replaced.end <= bytes.start {
                        Some(
                            bytes.start - replaced.len() + replacement.len()
                                ..bytes.end - replaced.len() + replacement.len(),
                        )
                    } else if bytes.end <= replaced.start {
                        Some(bytes.clone())
                    } else {
                        None
                    };
                    let found = document.current_range(1, 1, range);
                    let place =
                        format!("{range:?} of {text:?} after {edited:?} -> {replacement:?}");
                    match found {
                        Some(found) => assert_eq!(
                            document.text.as_ref().unwrap().range(found).ok(),
                            expected,
                            "{place}"
                        ),
                        None => assert!(expected.is_none() || joins, "{place}"),
                    }
                }
            }
        }
    }
}
