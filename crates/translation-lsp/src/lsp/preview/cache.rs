use super::{Markup, Preview, fenced_text, header, plain_text, shown};
use crate::{config::limits::TranslationLimits, providers::Output, store::Key};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range, Uri};

/// What a cache entry holds.
pub(crate) enum Stored<C> {
    /// A completed translation, kept for the output it was made for.
    Translation(Output, Preview<C>),
    /// A failed range translation. Its Hover shows the message.
    Failure {
        context: C,
        range: Range,
        message: String,
    },
    /// An automatic range translation that found its text already in the
    /// target language. It shows nothing, and only automatic requests reuse
    /// it. `keys` are those the result is stored under.
    SameLanguage {
        context: C,
        range: Range,
        keys: Vec<Key>,
    },
}

impl<C: PartialEq> Stored<C> {
    /// A failure whose message is cut to a size that a Hover can show.
    pub fn failure(context: C, range: Range, message: &str) -> Self {
        Self::Failure {
            context,
            range,
            message: message.chars().take(2048).collect(),
        }
    }

    fn output(&self) -> Output {
        match self {
            Self::Translation(output, _) => *output,
            Self::Failure { .. } | Self::SameLanguage { .. } => Output::Range,
        }
    }

    fn range(&self) -> Range {
        match self {
            Self::Translation(_, preview) => preview.range,
            Self::Failure { range, .. } | Self::SameLanguage { range, .. } => *range,
        }
    }

    fn context(&self) -> &C {
        match self {
            Self::Translation(_, preview) => &preview.context,
            Self::Failure { context, .. } | Self::SameLanguage { context, .. } => context,
        }
    }

    fn text(&self) -> &str {
        match self {
            Self::Translation(_, preview) => &preview.translation,
            Self::Failure { message, .. } => message,
            Self::SameLanguage { .. } => "",
        }
    }

    fn translation(&self, output: Output) -> Option<&Preview<C>> {
        match self {
            Self::Translation(stored, preview) if *stored == output => Some(preview),
            _ => None,
        }
    }

    /// The keys the result is stored under.
    fn keys(&self) -> &[Key] {
        match self {
            Self::Translation(_, preview) => &preview.keys,
            Self::SameLanguage { keys, .. } => keys,
            Self::Failure { .. } => &[],
        }
    }

    fn is_failure(&self) -> bool {
        matches!(self, Self::Failure { .. })
    }

    fn is_same_language(&self) -> bool {
        matches!(self, Self::SameLanguage { .. })
    }
}

/// What a CodeLens tells of a range.
pub(crate) enum Lens<'a> {
    Translated(&'a str),
    Failed,
}

struct Entry<C> {
    uri: Uri,
    ticket: u64,
    stored: Stored<C>,
    used: u64,
}

pub(crate) struct Cache<C> {
    entries: Vec<Entry<C>>,
    clock: u64,
    count_limit: usize,
    byte_limit: usize,
}

pub(crate) fn contains(outer: Range, inner: Range) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

pub(crate) fn overlaps(a: Range, b: Range) -> bool {
    if a.start == a.end {
        return b.start <= a.start && a.start <= b.end;
    }
    if b.start == b.end {
        return a.start <= b.start && b.start <= a.end;
    }
    a.start < b.end && b.start < a.end
}

impl<C: Clone + PartialEq> Cache<C> {
    #[cfg(test)]
    pub fn latest(&self, uri: &Uri) -> Option<&Preview<C>> {
        self.entries
            .iter()
            .filter(|e| &e.uri == uri)
            .filter_map(|e| match &e.stored {
                Stored::Translation(_, preview) => Some((e.ticket, preview)),
                Stored::Failure { .. } | Stored::SameLanguage { .. } => None,
            })
            .max_by_key(|(ticket, _)| *ticket)
            .map(|(_, preview)| preview)
    }
    #[cfg(test)]
    pub fn successes(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| !e.stored.is_failure())
            .count()
    }

    pub fn new(limits: TranslationLimits) -> Self {
        Self {
            entries: Vec::new(),
            clock: 0,
            count_limit: limits.cache.entries,
            byte_limit: limits
                .cache
                .capacity_kib
                .bytes()
                .max(limits.output_kib.bytes()),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has_document(&self, uri: &Uri) -> bool {
        self.entries.iter().any(|e| &e.uri == uri)
    }

    /// Removes the results of `uri`, returning the keys they are stored
    /// under.
    pub fn remove_document(&mut self, uri: &Uri) -> Vec<Key> {
        self.remove_where(|e| &e.uri == uri)
    }

    /// Removes the results of exactly `range`, returning the keys they are
    /// stored under.
    pub fn remove_range(&mut self, uri: &Uri, range: Range) -> Vec<Key> {
        self.remove_where(|e| &e.uri == uri && e.stored.range() == range)
    }

    fn remove_where(&mut self, removed: impl Fn(&Entry<C>) -> bool) -> Vec<Key> {
        let mut keys = Vec::new();
        self.entries.retain(|e| {
            if removed(e) {
                keys.extend_from_slice(e.stored.keys());
                false
            } else {
                true
            }
        });
        keys
    }

    pub fn retain_context(&mut self, context: &C) {
        self.entries.retain(|e| e.stored.context() == context);
    }

    fn newest(&self, uri: &Uri, select: impl Fn(&Stored<C>) -> bool) -> Option<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| &e.uri == uri && select(&e.stored))
            .max_by_key(|(_, e)| e.ticket)
            .map(|(i, _)| i)
    }

    fn use_entry(&mut self, index: usize) -> &Stored<C> {
        self.clock += 1;
        self.entries[index].used = self.clock;
        &self.entries[index].stored
    }

    /// Marks the newest entry of `uri` that `select` accepts as used.
    fn use_newest(&mut self, uri: &Uri, select: impl Fn(&Stored<C>) -> bool) -> Option<&Stored<C>> {
        let index = self.newest(uri, select)?;
        Some(self.use_entry(index))
    }

    pub fn exact(&mut self, uri: &Uri, range: Range, output: Output) -> Option<Preview<C>> {
        self.use_newest(uri, |stored| {
            stored
                .translation(output)
                .is_some_and(|preview| preview.range == range)
        })
        .and_then(|stored| stored.translation(output))
        .cloned()
    }

    /// Whether the newest range result of exactly `range` found its text
    /// already in the target language.
    pub fn same_language(&mut self, uri: &Uri, range: Range) -> bool {
        self.use_newest(uri, |stored| {
            stored.output() == Output::Range && stored.range() == range
        })
        .is_some_and(|stored| matches!(stored, Stored::SameLanguage { .. }))
    }

    /// The newest range translation or failure of exactly `range`: `Ok`
    /// with a translation, `Err` with the reason of a failure.
    pub fn result(&mut self, uri: &Uri, range: Range) -> Option<Result<&Preview<C>, &str>> {
        let stored = self.use_newest(uri, |stored| {
            stored.range() == range
                && matches!(
                    stored,
                    Stored::Translation(Output::Range, _) | Stored::Failure { .. }
                )
        })?;
        match stored {
            Stored::Translation(_, preview) => Some(Ok(preview)),
            Stored::Failure { message, .. } => Some(Err(message)),
            Stored::SameLanguage { .. } => None,
        }
    }

    /// The message that shows the newest range translation or failure of
    /// exactly `range`: `Ok` with a translation, `Err` with the reason of a
    /// failure.
    pub fn message(&mut self, uri: &Uri, range: Range) -> Option<Result<String, String>> {
        self.result(uri, range)
            .map(|result| result.map(Preview::message).map_err(plain_text))
    }

    /// The range translations and failures of a document, by their position.
    pub fn lenses(&self, uri: &Uri) -> Vec<(Range, Lens<'_>)> {
        let mut lenses: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| &entry.uri == uri)
            .filter_map(|entry| match &entry.stored {
                Stored::Translation(Output::Range, preview) => {
                    Some((preview.range, Lens::Translated(preview.lens_text())))
                }
                Stored::Failure { range, .. } => Some((*range, Lens::Failed)),
                Stored::Translation(Output::Document, _) | Stored::SameLanguage { .. } => None,
            })
            .collect();
        lenses.sort_by_key(|(range, _)| range.start);
        lenses
    }

    /// Whether no range result of `uri`, whether a translation, a failure, or
    /// a text found in the target language, overlaps `range`.
    pub fn is_vacant(&self, uri: &Uri, range: Range) -> bool {
        !self.entries.iter().any(|e| {
            &e.uri == uri && e.stored.output() == Output::Range && overlaps(e.stored.range(), range)
        })
    }

    /// Whether a translation of `bytes` fits in without dropping another
    /// result.
    pub fn has_room(&self, bytes: usize) -> bool {
        let used: usize = self.entries.iter().map(|e| e.stored.text().len()).sum();
        self.entries.len() < self.count_limit && used + bytes <= self.byte_limit
    }

    pub fn failed_range(&self, uri: &Uri, position: Position) -> Option<Range> {
        self.newest_failure_at(uri, position)
            .map(|index| self.entries[index].stored.range())
    }

    fn newest_translation_at(
        &self,
        uri: &Uri,
        position: Position,
        eligible: impl Fn(&Preview<C>) -> bool,
    ) -> Option<usize> {
        self.newest(uri, |stored| {
            stored.translation(Output::Range).is_some_and(|preview| {
                contains(preview.range, Range::new(position, position)) && eligible(preview)
            })
        })
    }

    fn newest_failure_at(&self, uri: &Uri, position: Position) -> Option<usize> {
        self.newest(uri, |stored| {
            stored.is_failure() && contains(stored.range(), Range::new(position, position))
        })
    }

    /// The range of the translation or failure that Hover would show from the cache.
    pub fn visible_range(
        &self,
        uri: &Uri,
        position: Position,
        eligible: impl Fn(&Preview<C>) -> bool,
    ) -> Option<Range> {
        let index = self
            .newest_translation_at(uri, position, eligible)
            .or_else(|| self.newest_failure_at(uri, position))?;
        Some(self.entries[index].stored.range())
    }

    /// The Hover of the newest range translation at `position` that `eligible`
    /// accepts, for a client that reads Markdown as `markup` says.
    pub fn hover(
        &mut self,
        uri: &Uri,
        position: Position,
        eligible: impl Fn(&Preview<C>) -> bool,
        markup: Markup,
    ) -> Option<Hover> {
        let index = self.newest_translation_at(uri, position, eligible)?;
        self.use_entry(index)
            .translation(Output::Range)?
            .hover(position, markup)
    }

    /// The Hover of the newest failure at `position`, in a client that draws
    /// theme icons if `icons`.
    pub fn failure_hover(&mut self, uri: &Uri, position: Position, icons: bool) -> Option<Hover> {
        let index = self.newest_failure_at(uri, position)?;
        let Stored::Failure { range, message, .. } = self.use_entry(index) else {
            return None;
        };
        Some(Hover {
            range: Some(*range),
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "{}\n\n{}\n\nTo retry, place the cursor in this range and run Retry translation of lines … from Code Actions.",
                    header("Translation LSP: translation failed", None, icons),
                    shown(fenced_text(message), icons)
                ),
            }),
        })
    }

    pub fn insert(&mut self, uri: Uri, ticket: u64, stored: Stored<C>) {
        let output = stored.output();
        let tab = output == Output::Document;
        let range = stored.range();
        let same_language = stored.is_same_language();
        let translation = matches!(stored, Stored::Translation(..));
        // A result that shows nothing never takes the place of one that shows
        // a translation: it neither keeps an older translation out nor
        // removes one. It keeps an older failure out, as any newer result does.
        if self.entries.iter().any(|e| {
            e.uri == uri
                && e.stored.output() == output
                && e.ticket > ticket
                && !e.stored.is_failure()
                && !(translation && e.stored.is_same_language())
                && (tab || contains(e.stored.range(), range))
        }) {
            return;
        }
        let failure = stored.is_failure();
        self.entries.retain(|e| {
            if e.uri != uri || e.stored.output() != output || e.ticket > ticket {
                return true;
            }
            if failure {
                return !e.stored.is_failure() || e.stored.range() != range;
            }
            if same_language {
                return !(e.stored.is_same_language() && e.stored.range() == range
                    || e.stored.is_failure() && contains(range, e.stored.range()));
            }
            !(tab || contains(range, e.stored.range()))
        });
        self.clock += 1;
        self.entries.push(Entry {
            uri,
            ticket,
            stored,
            used: self.clock,
        });
        while self.entries.len() > self.count_limit
            || self
                .entries
                .iter()
                .map(|e| e.stored.text().len())
                .sum::<usize>()
                > self.byte_limit
        {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.used)
                .unwrap()
                .0;
            self.entries.remove(oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockContext;

    fn range(start: u32, end: u32) -> Range {
        Range::new(Position::new(0, start), Position::new(0, end))
    }

    fn preview(range: Range, text: &str) -> Preview<MockContext> {
        Preview::new(MockContext, range, text.into(), false, None)
    }

    fn value(hover: Hover) -> String {
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("markup expected")
        };
        markup.value
    }

    #[test]
    fn ranges_survive_reverse_completion_failures_and_tab_results() {
        let uri: Uri = "file:///ranges.md".parse().unwrap();
        let mut cache = Cache::new(TranslationLimits::default());
        cache.insert(
            uri.clone(),
            2,
            Stored::Translation(Output::Range, preview(range(0, 10), "new")),
        );
        cache.insert(
            uri.clone(),
            1,
            Stored::Translation(Output::Range, preview(range(0, 5), "old")),
        );
        assert!(cache.exact(&uri, range(0, 5), Output::Range).is_none());
        cache.insert(
            uri.clone(),
            3,
            Stored::Translation(Output::Range, preview(range(12, 20), "second")),
        );
        cache.insert(
            uri.clone(),
            4,
            Stored::failure(MockContext, range(0, 10), "failure"),
        );
        cache.insert(
            uri.clone(),
            5,
            Stored::Translation(Output::Document, preview(range(0, 30), "whole document")),
        );
        assert!(
            value(
                cache
                    .hover(&uri, Position::new(0, 3), |_| true, Markup::default())
                    .unwrap()
            )
            .contains("new")
        );
        assert!(
            value(
                cache
                    .hover(&uri, Position::new(0, 15), |_| true, Markup::default())
                    .unwrap()
            )
            .contains("second")
        );
        assert!(
            cache
                .hover(&uri, Position::new(0, 25), |_| true, Markup::default())
                .is_none()
        );
        cache.insert(
            uri.clone(),
            6,
            Stored::Translation(Output::Range, preview(range(0, 10), "retry")),
        );
        assert!(
            cache
                .failure_hover(&uri, Position::new(0, 3), false)
                .is_none()
        );
        assert_eq!(
            cache
                .exact(&uri, range(0, 10), Output::Range)
                .unwrap()
                .translation,
            "retry"
        );
        cache.remove_document(&uri);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn global_lru_enforces_entries_bytes_and_single_output_floor() {
        let mut limits: TranslationLimits = serde_json::from_value(serde_json::json!({
            "output_kib": 2, "cache": {"entries": 2, "capacity_kib": 1}
        }))
        .unwrap();
        let a: Uri = "file:///a".parse().unwrap();
        let b: Uri = "file:///b".parse().unwrap();
        let mut cache = Cache::new(limits);
        cache.insert(
            a.clone(),
            1,
            Stored::Translation(Output::Range, preview(range(0, 1), &"a".repeat(2048))),
        );
        assert!(cache.latest(&a).is_some());
        cache.insert(
            b.clone(),
            2,
            Stored::Translation(Output::Range, preview(range(0, 1), "b")),
        );
        assert!(cache.latest(&a).is_none());
        limits.cache.capacity_kib = 16.try_into().unwrap();
        let mut cache = Cache::new(limits);
        cache.insert(
            a.clone(),
            1,
            Stored::Translation(Output::Range, preview(range(0, 1), "a")),
        );
        cache.insert(
            b.clone(),
            2,
            Stored::Translation(Output::Range, preview(range(0, 1), "b")),
        );
        cache.exact(&a, range(0, 1), Output::Range).unwrap();
        cache.insert(
            a.clone(),
            3,
            Stored::Translation(Output::Range, preview(range(2, 3), "c")),
        );
        assert!(cache.latest(&b).is_none());
        assert!(cache.exact(&a, range(0, 1), Output::Range).is_some());
    }

    #[test]
    fn same_language_results_show_nothing_and_go_with_their_context_and_document() {
        let uri: Uri = "file:///same.rs".parse().unwrap();
        let mut cache = Cache::new(TranslationLimits::default());
        let same = |context: u8| Stored::SameLanguage {
            context,
            range: range(0, 10),
            keys: Vec::new(),
        };
        cache.insert(uri.clone(), 1, same(1));
        assert!(cache.same_language(&uri, range(0, 10)));
        assert!(!cache.same_language(&uri, range(0, 9)));
        assert!(cache.lenses(&uri).is_empty());
        assert!(cache.message(&uri, range(0, 10)).is_none());
        assert!(
            cache
                .hover(&uri, Position::new(0, 3), |_| true, Markup::default())
                .is_none()
        );
        assert!(cache.exact(&uri, range(0, 10), Output::Range).is_none());
        // A later translation of the range replaces it.
        cache.insert(
            uri.clone(),
            2,
            Stored::Translation(
                Output::Range,
                Preview::new(1, range(0, 10), "訳".into(), false, None),
            ),
        );
        assert!(!cache.same_language(&uri, range(0, 10)));
        cache.insert(uri.clone(), 3, same(1));
        cache.retain_context(&2);
        assert!(!cache.same_language(&uri, range(0, 10)));
        cache.insert(uri.clone(), 4, same(2));
        cache.remove_document(&uri);
        assert!(!cache.same_language(&uri, range(0, 10)));
    }

    #[test]
    fn same_language_results_neither_remove_nor_keep_out_translations() {
        let uri: Uri = "file:///same.rs".parse().unwrap();
        let mut cache = Cache::new(TranslationLimits::default());
        let translation = |range| Stored::Translation(Output::Range, preview(range, "訳"));
        let same = |range| Stored::SameLanguage {
            context: MockContext,
            range,
            keys: Vec::new(),
        };
        // An older translation of a part comes in after a newer result that
        // shows nothing; one that was there stays.
        cache.insert(uri.clone(), 2, same(range(0, 20)));
        cache.insert(uri.clone(), 1, translation(range(0, 5)));
        cache.insert(uri.clone(), 3, translation(range(6, 10)));
        cache.insert(uri.clone(), 4, same(range(0, 20)));
        assert_eq!(cache.lenses(&uri).len(), 2);
        assert!(cache.same_language(&uri, range(0, 20)));
        // It replaces only the same result of the same range, and failures it
        // covers.
        cache.insert(
            uri.clone(),
            5,
            Stored::failure(MockContext, range(12, 15), "failure"),
        );
        cache.insert(uri.clone(), 6, same(range(0, 20)));
        assert!(
            cache
                .failure_hover(&uri, Position::new(0, 13), false)
                .is_none()
        );
        assert_eq!(
            cache
                .entries
                .iter()
                .filter(|e| e.stored.is_same_language())
                .count(),
            1
        );
        assert_eq!(cache.lenses(&uri).len(), 2);
    }

    #[test]
    fn a_newer_same_language_result_keeps_an_older_failure_out_in_either_order() {
        let uri: Uri = "file:///same.rs".parse().unwrap();
        for reverse in [false, true] {
            let mut cache = Cache::new(TranslationLimits::default());
            let same = Stored::SameLanguage {
                context: MockContext,
                range: range(0, 20),
                keys: Vec::new(),
            };
            let failed = Stored::failure(MockContext, range(0, 5), "Old request failed");
            if reverse {
                cache.insert(uri.clone(), 2, same);
                cache.insert(uri.clone(), 1, failed);
            } else {
                cache.insert(uri.clone(), 1, failed);
                cache.insert(uri.clone(), 2, same);
            }
            assert!(
                cache
                    .failure_hover(&uri, Position::new(0, 3), false)
                    .is_none(),
                "reverse={reverse}"
            );
            assert!(cache.lenses(&uri).is_empty());
            assert!(cache.same_language(&uri, range(0, 20)));
        }
    }
}
