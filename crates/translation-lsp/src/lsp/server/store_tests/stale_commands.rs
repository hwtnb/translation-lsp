//! CodeLens commands of an older version, which Zed sends from a line that it
//! kept across an edit because the line shows the same text.

use super::{
    super::menu::{Card, DELETE, RETRY},
    *,
};

/// A line added at the start of a document.
fn line_added() -> Value {
    json!([{"range": line(0, 0), "text": "fn z() {}\n"}])
}

impl Editor {
    /// Sends `changes` as the next version of `uri`.
    fn change(&mut self, uri: &Uri, changes: Value) {
        let version = self.server.documents[uri].version + 1;
        self.server
            .notification(
                &self.connection,
                lsp_server::Notification::new(
                    DidChangeTextDocument::METHOD.into(),
                    json!({
                        "textDocument": {"uri": uri, "version": version},
                        "contentChanges": changes
                    }),
                ),
            )
            .unwrap();
    }

    /// The CodeLenses of `uri`, after the stored translations that the
    /// requests look up enter the cache.
    fn code_lenses_found(&mut self, uri: &Uri) -> Vec<CodeLens> {
        let request = || parse(json!({"textDocument": {"uri": uri}})).unwrap();
        loop {
            let started = self.server.lookups.started();
            let lenses = self.server.code_lenses(request()).unwrap();
            if self.server.lookups.started() == started {
                return lenses;
            }
            let looked = self.server.lookups.results().recv_timeout(WAIT).unwrap();
            self.server.take_stored(looked).unwrap();
        }
    }

    /// The target of the CodeLens of `uri` with `command` on `line`.
    fn lens_target(&mut self, uri: &Uri, line: u32, command: &str) -> TranslationTarget {
        let lenses = self.code_lenses_found(uri);
        let found = lenses
            .into_iter()
            .filter_map(|lens| lens.command.filter(|_| lens.range.start.line == line))
            .find(|found| found.command == command)
            .unwrap_or_else(|| panic!("no {command} on line {line}"));
        serde_json::from_value(found.arguments.unwrap().remove(0)).unwrap()
    }

    fn run(&mut self, command: &str, target: &TranslationTarget) -> RequestResult<Option<Value>> {
        let id = self.id();
        self.server.execute_command(
            &self.connection,
            id,
            parse(json!({"command": command, "arguments": [target]})).unwrap(),
        )
    }

    /// Answers the card that the server shows next with `button`, and gives
    /// the target that its buttons act on.
    fn answer_card(&mut self, button: &str) -> TranslationTarget {
        let card = loop {
            if let Message::Request(request) = self.client.receiver.recv_timeout(WAIT).unwrap()
                && request.method == "window/showMessageRequest"
            {
                break request;
            }
        };
        let Some((Card::Translation(target), _)) = self.server.cards.get(&card.id) else {
            panic!("not a translation card");
        };
        let target = target.clone();
        if let Some((Purpose::Card, response)) = self
            .server
            .ui
            .response(Response::new_ok(card.id, json!({"title": button})))
        {
            self.server
                .card_response(&self.connection, response)
                .unwrap();
        }
        target
    }
}

fn refusal(result: RequestResult<Option<Value>>) -> String {
    let Err(error) = result else {
        panic!("not refused");
    };
    assert_eq!(error.code, ErrorCode::ContentModified as i32);
    error.message
}

#[test]
fn translate_from_an_older_line_translates_the_comment_where_it_is_now() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    let translate = editor.lens_target(&uri, 0, LENS_TRANSLATE_COMMAND);
    editor.change(&uri, line_added());
    assert_eq!(
        editor.run(LENS_TRANSLATE_COMMAND, &translate).unwrap(),
        Some(Value::Null)
    );
    editor.finish_next();
    assert_eq!(provider.sent(), ["Alpha comment."]);
    let preview = editor
        .server
        .cache
        .exact(&uri, line(1, 17), Output::Range)
        .unwrap();
    assert!(preview.is_comment);
    assert_eq!(editor.server.cache.lenses(&uri).len(), 1);
}

#[test]
fn a_translation_line_from_before_an_edit_shows_its_card_where_the_comment_is_now() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    let show = editor.lens_target(&uri, 0, SHOW_COMMAND);
    editor.change(&uri, line_added());
    // The CodeLenses of the new version show the stored translation again,
    // as the line that Zed kept.
    editor.code_lenses_found(&uri);
    let moved = line(1, 17);
    assert!(
        editor
            .server
            .cache
            .exact(&uri, moved, Output::Range)
            .is_some()
    );

    assert_eq!(editor.run(SHOW_COMMAND, &show).unwrap(), Some(Value::Null));
    let card = editor.answer_card(RETRY);
    assert_eq!((card.version, card.range), (2, moved));
    editor.finish_next();
    assert_eq!(provider.sent(), ["Alpha comment.", "Alpha comment."]);
    assert!(
        editor
            .server
            .cache
            .exact(&uri, moved, Output::Range)
            .is_some()
    );

    assert_eq!(editor.run(SHOW_COMMAND, &show).unwrap(), Some(Value::Null));
    let card = editor.answer_card(DELETE);
    assert_eq!((card.version, card.range), (2, moved));
    assert!(
        editor
            .server
            .cache
            .exact(&uri, moved, Output::Range)
            .is_none()
    );
    assert_eq!(
        refusal(editor.run(SHOW_COMMAND, &show)),
        "The translation is no longer available"
    );
}

/// The texts that a `Translate` line of version 1 of a document translates
/// after `changes`: none if it is refused.
fn translated_after(name: &str, text: &str, line: u32, changes: Value) -> Vec<String> {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open(name, "rust", text);
    let translate = editor.lens_target(&uri, line, LENS_TRANSLATE_COMMAND);
    editor.change(&uri, changes);
    match editor.run(LENS_TRANSLATE_COMMAND, &translate) {
        Ok(_) => {
            editor.finish_next();
            let (range, _) = editor.server.cache.lenses(&uri).remove(0);
            let document = editor.server.documents.get_mut(&uri).unwrap();
            assert_eq!(
                document.comment_starting_at(&uri, range.start, WAIT),
                Some(range)
            );
        }
        Err(error) => {
            assert_eq!(
                (error.code, error.message.as_str()),
                (
                    ErrorCode::ContentModified as i32,
                    "Translation target belongs to an older document"
                )
            );
        }
    }
    provider.sent()
}

#[test]
fn translate_from_before_an_edit_of_its_comment_translates_the_comment_as_it_is_now() {
    let edit = |start: (u32, u32), end: (u32, u32), text: &str| json!([{"range": Range::new(Position::new(start.0, start.1), Position::new(end.0, end.1)), "text": text}]);
    // The comment written after its line appeared.
    assert_eq!(
        translated_after(
            "typed.rs",
            "// H\nfn a() {}\n",
            0,
            edit((0, 4), (0, 4), "ello")
        ),
        ["Hello"]
    );
    // A word replaced, and a line added at its end.
    assert_eq!(
        translated_after("word.rs", CODE, 0, edit((0, 3), (0, 8), "Gamma")),
        ["Gamma comment."]
    );
    assert_eq!(
        translated_after("added.rs", CODE, 0, edit((0, 17), (0, 17), "\n// More.")),
        ["Alpha comment.\nMore."]
    );
    // A comment line added above it, and a deletion across its start.
    assert!(translated_after("above.rs", CODE, 2, edit((2, 0), (2, 0), "// Zeta.\n")).is_empty());
    assert!(translated_after("across.rs", CODE, 2, edit((1, 5), (2, 3), "")).is_empty());
}

#[test]
fn a_command_from_before_an_edit_of_its_comment_is_refused() {
    const OLDER: &str = "Translation target belongs to an older document";
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);

    // A word of the comment changed: Retry follows the whole range.
    let uri = editor.open("a.rs", "rust", CODE);
    let translate = editor.lens_target(&uri, 0, LENS_TRANSLATE_COMMAND);
    editor.change(
        &uri,
        json!([{"range": Range::new(Position::new(0, 3), Position::new(0, 8)), "text": "Gamma"}]),
    );
    assert_eq!(refusal(editor.run(LENS_RETRY_COMMAND, &translate)), OLDER);

    // A comment line added before the second comment joins the two, while
    // the first comment is still whole.
    let uri = editor.open("b.rs", "rust", CODE);
    let first = editor.lens_target(&uri, 0, LENS_TRANSLATE_COMMAND);
    let second = editor.lens_target(&uri, 2, LENS_TRANSLATE_COMMAND);
    editor.change(
        &uri,
        json!([{"range": line(2, 0), "text": "// Zeta comment.\n"}]),
    );
    assert_eq!(refusal(editor.run(LENS_TRANSLATE_COMMAND, &second)), OLDER);
    assert!(provider.sent().is_empty());
    assert_eq!(
        editor.run(LENS_TRANSLATE_COMMAND, &first).unwrap(),
        Some(Value::Null)
    );
    editor.finish_next();
    assert_eq!(provider.sent(), ["Alpha comment."]);

    // The line of a translation whose comment changed shows no card.
    let uri = editor.open("c.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    let show = editor.lens_target(&uri, 0, SHOW_COMMAND);
    editor.change(
        &uri,
        json!([{"range": Range::new(Position::new(0, 3), Position::new(0, 8)), "text": "Gamma"}]),
    );
    editor.code_lenses_found(&uri);
    assert_eq!(
        refusal(editor.run(SHOW_COMMAND, &show)),
        "The translation is no longer available"
    );
    assert!(editor.server.cards.is_empty());
}

#[test]
fn a_markdown_translation_line_from_before_a_paragraph_was_added_shows_its_card() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.md", "markdown", "Read the guide.\n");
    editor.translate(&uri, line(0, 15), PendingReply::Command { notice: false });
    let show = editor.lens_target(&uri, 0, SHOW_COMMAND);
    editor.change(
        &uri,
        json!([{"range": line(0, 0), "text": "New paragraph.\n\n"}]),
    );
    // The selection is translated again where it is now, as the stored
    // translation, which gives its line the same text.
    let moved = line(2, 15);
    editor.translate(&uri, moved, PendingReply::Command { notice: false });
    assert_eq!(provider.sent(), ["Read the guide."]);
    assert_eq!(editor.run(SHOW_COMMAND, &show).unwrap(), Some(Value::Null));
    let card = editor.answer_card(DELETE);
    assert_eq!((card.version, card.range), (2, moved));
    assert!(
        editor
            .server
            .cache
            .exact(&uri, moved, Output::Range)
            .is_none()
    );
}

#[test]
fn code_action_commands_from_before_an_edit_are_refused_as_before() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.hover(&uri, FIRST);
    let old = editor.target(&uri, line(0, 17));
    editor.change(&uri, line_added());
    editor.code_lenses_found(&uri);
    for command in [COMMAND, RETRY_COMMAND] {
        assert_eq!(
            refusal(editor.run(command, &old)),
            "Translation target belongs to an older document"
        );
    }
    // A deletion of an older version deletes nothing.
    assert_eq!(editor.run(DELETE_COMMAND, &old).unwrap(), Some(Value::Null));
    assert!(
        editor
            .server
            .cache
            .exact(&uri, line(1, 17), Output::Range)
            .is_some()
    );
    assert_eq!(provider.sent(), ["Alpha comment."]);
}

#[test]
fn translate_in_tab_from_an_older_line_translates_the_document_as_it_is_now() {
    const OLDER: &str = "Translation target belongs to an older document";
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.md", "markdown", "Read the guide.\n");
    let tab = editor.lens_target(&uri, 0, LENS_DOCUMENT_COMMAND);
    // The Code Action of the whole document and of a selection.
    let whole = editor.target(&uri, tab.range);
    let selection = editor.target(&uri, line(0, 4));
    editor.change(
        &uri,
        json!([{"range": line(0, 0), "text": "New paragraph.\n\n"}]),
    );
    for target in [&whole, &selection] {
        assert_eq!(refusal(editor.run(DOCUMENT_COMMAND, target)), OLDER);
    }
    assert_eq!(
        editor.run(LENS_DOCUMENT_COMMAND, &tab).unwrap(),
        Some(Value::Null)
    );
    editor.finish_next();
    assert_eq!(provider.sent(), ["New paragraph.\n\nRead the guide.\n"]);
    let now = editor.server.documents[&uri].full_range().unwrap();
    assert_eq!(now, Range::new(Position::new(0, 0), Position::new(3, 0)));
    assert!(
        editor
            .server
            .cache
            .exact(&uri, now, Output::Document)
            .is_some()
    );

    // A document whose synchronization was lost, and one opened again.
    editor.change(&uri, json!([{"range": line(9, 0), "text": "x"}]));
    assert_eq!(refusal(editor.run(LENS_DOCUMENT_COMMAND, &tab)), OLDER);
    editor.open("a.md", "markdown", "Read the guide.\n");
    assert_eq!(refusal(editor.run(LENS_DOCUMENT_COMMAND, &tab)), OLDER);
    assert_eq!(provider.sent().len(), 1);
}
