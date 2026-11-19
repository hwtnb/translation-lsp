//! CodeLenses of comments that show stored translations.

use std::collections::BTreeMap;

use super::*;

const TRANSLATE: [&str; 2] = ["Translate", "Settings"];
const TRANSLATED: [&str; 2] = ["訳: Alpha comment.", "Settings"];
/// The comments of [`CODE`].
const FIRST_COMMENT: Range = Range {
    start: Position {
        line: 0,
        character: 0,
    },
    end: Position {
        line: 0,
        character: 17,
    },
};
const SECOND_COMMENT: Range = Range {
    start: Position {
        line: 2,
        character: 0,
    },
    end: Position {
        line: 2,
        character: 17,
    },
};

impl Editor {
    /// The titles of the CodeLenses of `uri` by their line. A lookup that the
    /// request starts is left in progress.
    fn lens_lines(&mut self, uri: &Uri) -> BTreeMap<u32, Vec<String>> {
        let mut lines = BTreeMap::<u32, Vec<String>>::new();
        let lenses = self
            .server
            .code_lenses(parse(json!({"textDocument": {"uri": uri}})).unwrap())
            .unwrap();
        for lens in lenses {
            lines
                .entry(lens.range.start.line)
                .or_default()
                .push(lens.command.unwrap().title);
        }
        lines
    }

    /// Ends the lookup in progress, putting what it found into the cache.
    fn take_stored(&mut self) {
        let looked = self.server.lookups.results().recv_timeout(WAIT).unwrap();
        self.server.take_stored(looked).unwrap();
    }

    /// How many lookups the server started.
    fn lookups(&self) -> u64 {
        self.server.lookups.started()
    }

    /// Whether Hover over `position` shows a result without a translation.
    fn hover_is_ready(&mut self, uri: &Uri, position: Position) -> bool {
        let id = self.id();
        let params = parse(json!({"textDocument": {"uri": uri}, "position": position})).unwrap();
        self.server
            .hover(id, params)
            .unwrap()
            .is_some_and(|hover| !hover.is_null())
    }
}

/// CodeLens titles by line for the two comments of [`CODE`].
fn lines(first: [&str; 2], second: [&str; 2]) -> BTreeMap<u32, Vec<String>> {
    BTreeMap::from([
        (0, first.map(String::from).to_vec()),
        (2, second.map(String::from).to_vec()),
    ])
}

#[test]
fn a_translation_of_the_same_comment_elsewhere_shows_in_its_code_lens() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
    editor.take_stored();
    editor.hover(&uri, FIRST);
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATE));
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
    assert_eq!(editor.lookups(), 2);
    assert!(editor.hover_is_ready(&uri, SECOND));
    assert_eq!(provider.sent(), ["Alpha comment."]);
    assert!(editor.server.scheduler.translations().is_empty());
}

#[test]
fn after_a_restart_with_disk_cache_code_lenses_show_stored_translations() {
    for enabled in [true, false] {
        let scratch = Scratch::new();
        let provider = Counter::default();
        let settings = json!({"disk_cache": {"enabled": enabled}});
        let mut editor = Editor::new(
            provider.clone(),
            settings.clone(),
            Some(scratch.translations()),
        );
        let uri = editor.open("a.rs", "rust", CODE);
        editor.hover(&uri, FIRST);
        drop(editor);

        let mut editor = Editor::new(provider.clone(), settings, Some(scratch.translations()));
        let uri = editor.open("a.rs", "rust", CODE);
        assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
        editor.take_stored();
        if !enabled {
            assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
            assert!(!editor.hover_is_ready(&uri, FIRST));
            continue;
        }
        assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
        // Hover shows it from the cache, and deleting it forgets it.
        assert!(editor.hover_is_ready(&uri, FIRST));
        assert_eq!(provider.sent().len(), 1);
        let preview = editor
            .server
            .cache
            .exact(&uri, FIRST_COMMENT, Output::Range)
            .unwrap();
        assert!(preview.is_comment);
        assert_eq!(preview.keys.len(), 2);
    }
}

#[test]
fn a_result_found_for_an_older_state_is_not_put_into_the_cache() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    // The document changes while the lookup is in progress.
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    editor.edit(&uri, CODE);
    editor.take_stored();
    assert!(editor.server.cache.lenses(&uri).is_empty());
    // CodeLenses of the new version look it up again.
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));

    // The provider becomes unavailable while the lookup is in progress.
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    provider.record().unavailable = true;
    editor.take_stored();
    assert!(editor.server.cache.lenses(&uri).is_empty());
    provider.record().unavailable = false;

    // A translation is stored while the lookup is in progress.
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    let other = editor.open("b.rs", "rust", "// Beta comment.\n");
    editor.hover(&other, FIRST);
    editor.take_stored();
    assert!(editor.server.cache.lenses(&uri).is_empty());
    assert_eq!(provider.sent(), ["Alpha comment.", "Beta comment."]);
}

#[test]
fn a_result_or_translation_that_came_meanwhile_keeps_its_place() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    // A retry of the second comment fails while the lookup is in progress,
    // which stores nothing.
    editor.lens_lines(&uri);
    provider.record().fail = Some("Alpha".into());
    editor.translate(&uri, SECOND_COMMENT, PendingReply::Retry { notice: false });
    editor.take_stored();
    assert_eq!(
        editor.lens_lines(&uri),
        BTreeMap::from([
            (0, TRANSLATED.map(String::from).to_vec()),
            (
                2,
                ["Translation failed", "Retry", "Settings"]
                    .map(String::from)
                    .to_vec()
            ),
        ])
    );

    // A retry in progress keeps its range too.
    provider.record().fail = None;
    provider.record().delay = Duration::from_millis(200);
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    let id = editor.id();
    let target = editor.target(&uri, SECOND_COMMENT);
    editor
        .server
        .queue(id, target, PendingReply::Retry { notice: false })
        .unwrap();
    editor.take_stored();
    assert!(
        editor
            .server
            .cache
            .exact(&uri, SECOND_COMMENT, Output::Range)
            .is_none()
    );
    assert_eq!(
        editor.lens_lines(&uri)[&2],
        ["Translating…", "Cancel", "Settings"]
    );
    editor.finish_next();
    assert_eq!(provider.sent().len(), 3);
}

#[test]
fn a_full_cache_drops_nothing_for_a_stored_translation() {
    let provider = Counter::default();
    let mut editor = Editor::new(
        provider.clone(),
        json!({"translation_limits": {"cache": {"entries": 1}}}),
        None,
    );
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATE));
}

#[test]
fn comments_without_stored_translations_are_not_looked_up_again_in_the_same_state() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
    assert_eq!(editor.lookups(), 1);
    // A request while a lookup is in progress asks for CodeLenses again when
    // it ends, rather than starting another.
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    assert_eq!(editor.lookups(), 2);
    editor.take_stored();
    editor.lens_lines(&uri);
    assert_eq!(editor.lookups(), 3);
    editor.take_stored();
    // Nothing is looked up when every comment has a result.
    editor.hover(&uri, FIRST);
    editor.hover(&uri, SECOND);
    editor.lens_lines(&uri);
    assert_eq!(editor.lookups(), 3);
    assert_eq!(provider.sent().len(), 1);
}

#[test]
fn deleting_a_translation_found_for_a_code_lens_sends_the_next_hover() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
    let target = editor.target(&uri, SECOND_COMMENT);
    editor.execute(DELETE_COMMAND, json!([target]));
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATE));
    editor.take_stored();
    editor.hover(&uri, SECOND);
    assert_eq!(provider.sent().len(), 2);
}

#[test]
fn nothing_is_looked_up_without_comment_lens_in_markdown_or_without_a_key() {
    for settings in [
        json!({"features": {"comment_lens": false}}),
        json!({"features": {"languages": {"rust": {"comment_lens": false}}}}),
    ] {
        let mut editor = Editor::new(Counter::default(), settings, None);
        let uri = editor.open("a.rs", "rust", CODE);
        editor.hover(&uri, FIRST);
        editor.edit(&uri, CODE);
        editor.lens_lines(&uri);
        assert_eq!(editor.lookups(), 0);
    }

    let mut editor = Editor::new(Counter::default(), json!({}), None);
    let uri = editor.open("a.md", "markdown", "Alpha comment.\n");
    editor.translate(&uri, line(0, 14), PendingReply::Command { notice: false });
    editor.edit(&uri, "Alpha comment.\n");
    editor.lens_lines(&uri);
    assert_eq!(editor.lookups(), 0);

    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    provider.record().unavailable = true;
    let shown = editor.lens_lines(&uri);
    assert!(
        shown.values().flatten().all(|title| title == "Settings"),
        "{shown:?}"
    );
    assert_eq!(editor.lookups(), 0);
}

impl Editor {
    /// Sends an edit of `uri` that keeps its version, which the server
    /// cannot apply, so that the document loses its text.
    fn break_sync(&mut self, uri: &Uri) {
        let version = self.server.documents[uri].version;
        self.server
            .notification(
                &self.connection,
                lsp_server::Notification::new(
                    DidChangeTextDocument::METHOD.into(),
                    json!({
                        "textDocument": {"uri": uri, "version": version},
                        "contentChanges": [{"text": CODE}]
                    }),
                ),
            )
            .unwrap();
        assert!(self.server.documents[uri].text().is_none());
    }

    /// The titles of every Code Action for `range`.
    fn action_titles(&mut self, uri: &Uri, range: Range) -> Vec<String> {
        self.server
            .code_actions(
                parse(json!({
                    "textDocument": {"uri": uri},
                    "range": range,
                    "context": {"diagnostics": []}
                }))
                .unwrap(),
            )
            .unwrap()
            .into_iter()
            .map(|action| action.title)
            .collect()
    }
}

#[test]
fn a_document_that_lost_its_text_gets_no_stored_translation() {
    let mut editor = Editor::new(Counter::default(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    editor.break_sync(&uri);
    editor.take_stored();
    assert!(editor.server.cache.lenses(&uri).is_empty());
    assert!(editor.lens_lines(&uri).is_empty());
    assert_eq!(editor.lookups(), 1);
}

#[test]
fn a_lookup_dropped_while_the_key_was_unreadable_runs_again_once_it_is_back() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    editor.lens_lines(&uri);
    provider.record().unavailable = true;
    editor.take_stored();
    // Nothing is looked up while the key cannot be read.
    editor.lens_lines(&uri);
    assert_eq!(editor.lookups(), 1);
    provider.record().unavailable = false;
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
    assert_eq!(editor.lookups(), 2);
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
}

#[test]
fn a_comment_left_out_for_a_canceled_translation_is_looked_up_afterwards() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    provider.record().delay = Duration::from_secs(2);
    let id = editor.id();
    let target = editor.target(&uri, SECOND_COMMENT);
    editor
        .server
        .queue(id, target, PendingReply::Retry { notice: false })
        .unwrap();
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(
        editor.lens_lines(&uri)[&2],
        ["Translating…", "Cancel", "Settings"]
    );
    let ticket = *editor
        .server
        .scheduler
        .translations()
        .keys()
        .next()
        .unwrap();
    editor.execute(CANCEL_COMMAND, json!([ticket]));
    editor.finish_next();
    assert!(editor.server.scheduler.translations().is_empty());
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATE));
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
}

#[test]
fn after_a_key_change_lenses_drop_the_old_results_and_look_up_with_the_new_key() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
    provider.record().account = 1;
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATE, TRANSLATE));
    editor.take_stored();
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATED));
    let preview = editor
        .server
        .cache
        .exact(&uri, SECOND_COMMENT, Output::Range)
        .unwrap();
    assert_eq!(preview.context, 1);
    assert_eq!(provider.sent().len(), 1);
}

#[test]
fn looking_up_again_after_a_store_reads_memory_only() {
    let scratch = Scratch::new();
    let mut editor = Editor::new(
        Counter::default(),
        json!({"disk_cache": {"enabled": true}}),
        Some(scratch.translations()),
    );
    let uri = editor.open("a.rs", "rust", "// Gamma comment.\nfn g() {}\n");
    editor.lens_lines(&uri);
    editor.take_stored();
    // Another translation is stored, and the document is looked up again.
    let other = editor.open("b.rs", "rust", "// Beta comment.\n");
    editor.hover(&other, FIRST);
    let reads = editor.server.store.reads();
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lookups(), 2);
    assert_eq!(editor.server.store.reads(), reads);
    // An edit reads the disk again.
    editor.edit(&uri, "// Gamma comment.\nfn g() {}\n");
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.lookups(), 3);
    assert!(editor.server.store.reads() > reads);
}

#[test]
fn a_lookup_finds_no_more_than_the_cache_keeps() {
    let mut editor = Editor::new(
        Counter::default(),
        json!({"translation_limits": {"cache": {"entries": 1}}}),
        None,
    );
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    editor.edit(&uri, CODE);
    let uses = editor.server.store.uses();
    editor.lens_lines(&uri);
    editor.take_stored();
    assert_eq!(editor.server.store.uses(), uses + 1);
    assert_eq!(editor.lens_lines(&uri), lines(TRANSLATED, TRANSLATE));
}

#[test]
fn the_lookup_thread_starts_with_the_first_lookup() {
    let mut editor = Editor::new(
        Counter::default(),
        json!({"features": {"comment_lens": false}}),
        None,
    );
    let uri = editor.open("a.rs", "rust", CODE);
    editor.lens_lines(&uri);
    assert!(!editor.server.lookups.is_running());

    let mut editor = Editor::new(Counter::default(), json!({}), None);
    assert!(!editor.server.lookups.is_running());
    let uri = editor.open("a.rs", "rust", "fn a() {}\n");
    editor.lens_lines(&uri);
    assert!(!editor.server.lookups.is_running());
    let uri = editor.open("b.rs", "rust", CODE);
    editor.lens_lines(&uri);
    assert!(editor.server.lookups.is_running());
    editor.take_stored();
}

#[test]
fn a_document_without_its_text_offers_only_deleting_everything() {
    const TEXT: &str = "Alpha comment.\n";
    let sentence = line(0, 14);
    // The translation is stored before the document loses its text.
    let mut editor = Editor::new(Counter::default(), json!({}), None);
    let uri = editor.open("a.md", "markdown", TEXT);
    editor.translate(&uri, sentence, PendingReply::Command { notice: false });
    editor.break_sync(&uri);
    assert_eq!(
        editor.action_titles(&uri, sentence),
        [menu::DELETE_ALL.to_owned()]
    );
    let menu = editor.server.menu_choices(Some(&uri)).unwrap();
    assert!(
        menu.iter()
            .all(|choice| choice.title != menu::DELETE_DOCUMENT)
    );
    assert!(menu.iter().any(|choice| choice.title == menu::DELETE_ALL));

    // The translation is canceled before it is stored.
    let provider = Counter::default();
    provider.record().delay = Duration::from_secs(2);
    let mut editor = Editor::new(provider, json!({}), None);
    let uri = editor.open("a.md", "markdown", TEXT);
    let id = editor.id();
    let target = editor.target(&uri, sentence);
    editor
        .server
        .queue(id, target, PendingReply::Command { notice: false })
        .unwrap();
    editor.break_sync(&uri);
    editor.finish_next();
    assert!(editor.server.store.is_empty());
    assert!(editor.action_titles(&uri, sentence).is_empty());
}
