//! The edits of the latest updates of a document, which tell where a range of
//! an older version is in the current text.

use std::collections::VecDeque;

use lsp_types::{Position, Range};

/// The most updates kept. Zed sends one for each typed character, so this
/// covers a few hundred of them.
const MAX_UPDATES: usize = 512;
/// The most edits kept across the updates, as one update can replace many
/// places.
pub(super) const MAX_EDITS: usize = 4096;

/// One replacement of an update, in the positions of the text it applied to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Edit {
    /// The range it replaced.
    range: Range,
    /// Where the inserted text ends afterwards, or `None` if the text after
    /// it no longer starts there: an inserted CR made one line ending with the
    /// LF after it.
    inserted_end: Option<Position>,
    /// Whether a CR before the range and an LF after it became one line
    /// ending, so that the start of the range no longer names the same place.
    joined: bool,
}

impl Edit {
    pub fn new(range: Range, inserted_end: Option<Position>, joined: bool) -> Self {
        Self {
            range,
            inserted_end,
            joined,
        }
    }

    /// `range` after this edit, or `None` if the edit touched it. An insertion
    /// at the start of `range` moves it; one at its end leaves it.
    fn map(&self, range: Range) -> Option<Range> {
        if self.range.end <= range.start {
            let inserted_end = self.inserted_end?;
            Some(Range::new(
                self.shift(range.start, inserted_end)?,
                self.shift(range.end, inserted_end)?,
            ))
        } else if range.end < self.range.start || (range.end == self.range.start && !self.joined) {
            Some(range)
        } else {
            None
        }
    }

    /// Where `position`, at or after the end of the replaced range, is after
    /// the edit. A position on the line of that end keeps its distance from
    /// it; one on a later line keeps its character.
    fn shift(&self, position: Position, inserted_end: Position) -> Option<Position> {
        let end = self.range.end;
        Some(if position.line == end.line {
            Position::new(
                inserted_end.line,
                inserted_end
                    .character
                    .checked_add(position.character - end.character)?,
            )
        } else {
            Position::new(
                inserted_end.line.checked_add(position.line - end.line)?,
                position.character,
            )
        })
    }
}

/// The edits of the updates since a version, in order.
#[derive(Default)]
pub(super) struct History {
    updates: VecDeque<Update>,
    /// The number of edits in `updates`.
    edits: usize,
}

struct Update {
    /// The version that the edits apply to.
    version: i32,
    edits: Vec<Edit>,
}

impl History {
    /// Forgets every update, so that no older version can be followed.
    pub fn clear(&mut self) {
        self.updates.clear();
        self.edits = 0;
    }

    /// Adds the edits that made the next version from `version`, the version
    /// that the last update made, or the first after [`Self::clear`]. The
    /// oldest updates are forgotten beyond the limits.
    pub fn record(&mut self, version: i32, edits: Vec<Edit>) {
        self.edits += edits.len();
        self.updates.push_back(Update { version, edits });
        while self.updates.len() > MAX_UPDATES || self.edits > MAX_EDITS {
            let Some(oldest) = self.updates.pop_front() else {
                break;
            };
            self.edits -= oldest.edits.len();
        }
    }

    /// `range` of `version` after every update since, if all of them are
    /// kept and none touched it.
    pub fn map(&self, version: i32, range: Range) -> Option<Range> {
        if range.start > range.end {
            return None;
        }
        let first = self
            .updates
            .iter()
            .position(|update| update.version == version)?;
        self.updates
            .range(first..)
            .flat_map(|update| &update.edits)
            .try_fold(range, |range, edit| edit.map(range))
    }
}

#[cfg(test)]
mod tests;
