use crossbeam_channel::Receiver;
use serde_json::json;

use super::*;

const WAIT: Duration = Duration::from_secs(5);
const COMMENT: &str = "// Retry only if the remote service\n// reports a temporary failure.";

struct Editor<P: TranslationProvider = MockProvider> {
    server: Server<P>,
    connection: Connection,
    client: Connection,
    events: Receiver<WorkerEvent<P::Context>>,
}

impl<P: TranslationProvider> Editor<P> {
    fn new(provider: P, settings: Settings) -> Self {
        let (connection, client) = Connection::memory();
        let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
        let server = Server::configured(
            finished,
            provider,
            settings,
            Ui::new(connection.sender.clone(), UiCapabilities::default()),
        )
        .unwrap();
        Self {
            server,
            connection,
            client,
            events,
        }
    }

    fn open(&mut self, uri: &Uri, language: &str, text: &str) {
        self.server.documents.insert(
            uri.clone(),
            Document::new(1, 1, language.into(), text.into()),
        );
    }

    fn execute(&mut self, id: i32, command: &str, arguments: Value) -> Option<Value> {
        self.server
            .execute_command(
                &self.connection,
                id.into(),
                parse(json!({
                    "command": command, "arguments": arguments
                }))
                .unwrap(),
            )
            .unwrap()
    }

    fn prompt(&self) -> Request {
        let Message::Request(prompt) = self.client.receiver.recv_timeout(WAIT).unwrap() else {
            panic!("Expected prompt");
        };
        assert_eq!(prompt.method, "window/showMessageRequest");
        prompt
    }

    fn answer_prompt(&mut self, prompt: Request, choice: Value) -> Value {
        if let Some((Purpose::Card, response)) =
            self.server.ui.response(Response::new_ok(prompt.id, choice))
        {
            self.server
                .card_response(&self.connection, response)
                .unwrap();
        }
        prompt.params
    }

    fn respond_to_prompt(&mut self, choice: Value) -> Value {
        self.answer_prompt(self.prompt(), choice)
    }

    fn finish_next(&mut self) -> bool {
        loop {
            match self.events.recv_timeout(WAIT).unwrap() {
                WorkerEvent::Started(ticket) => self.server.scheduler.mark_running(ticket),
                WorkerEvent::Finished(completion) => {
                    let translation = matches!(completion, Finished::Translation(..));
                    self.server.finish(&self.connection, completion).unwrap();
                    return translation;
                }
            }
        }
    }

    fn reply(&mut self, id: i32) -> Value {
        let Message::Response(response) = self.client.receiver.recv_timeout(WAIT).unwrap() else {
            panic!("Expected command or Hover response");
        };
        assert_eq!(response.id, id.into());
        response.response_result.unwrap()
    }

    fn hover(&mut self, id: i32, uri: &Uri, position: Position) -> Option<Value> {
        self.server
            .hover(
                id.into(),
                parse(json!({
                    "textDocument": {"uri": uri}, "position": position
                }))
                .unwrap(),
            )
            .unwrap()
    }

    fn lenses(&mut self, uri: &Uri) -> Vec<Command> {
        self.server
            .code_lenses(parse(json!({"textDocument": {"uri": uri}})).unwrap())
            .unwrap()
            .into_iter()
            .filter_map(|lens| lens.command)
            .collect()
    }

    fn delete_actions(&mut self, uri: &Uri, selection: Range) -> Vec<Command> {
        self.server
            .code_actions(
                parse(json!({
                    "textDocument": {"uri": uri}, "range": selection, "context": {"diagnostics": []}
                }))
                .unwrap(),
            )
            .unwrap()
            .into_iter()
            .filter_map(|action| action.command)
            .filter(|command| command.command.starts_with("translation-lsp.delete-"))
            .collect()
    }

    fn target(&self, uri: &Uri, range: Range) -> TranslationTarget {
        let document = &self.server.documents[uri];
        TranslationTarget {
            uri: uri.clone(),
            version: document.version,
            session: document.session,
            range,
            target_language: self.server.settings.target_language.clone(),
        }
    }
}

fn line(line: u32, len: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, len))
}

fn cursor(line: u32, column: u32) -> Range {
    let position = Position::new(line, column);
    Range::new(position, position)
}

fn comment_range(start: u32) -> Range {
    let end = COMMENT.lines().last().unwrap().len() as u32;
    Range::new(Position::new(start, 0), Position::new(start + 1, end))
}

fn translation(output: Output, range: Range) -> Stored<MockContext> {
    Stored::Translation(
        output,
        Preview::new(MockContext, range, "訳文".into(), false, None),
    )
}

#[test]
fn deleting_one_comment_removes_its_lens_and_the_next_hover_translates_it_again() {
    let uri = "file:///two.rs".parse().unwrap();
    let mut editor = Editor::new(MockProvider::default(), Settings::default());
    editor.open(&uri, "rust", &format!("{COMMENT}\n\n{COMMENT}"));
    let a = comment_range(0);
    let b = comment_range(3);
    for (id, range) in [(1, a), (2, b)] {
        assert!(editor.hover(id, &uri, range.start).is_none());
        assert!(editor.finish_next());
        let hover = editor.reply(id);
        assert_eq!(hover["range"], json!(range));
        assert!(
            hover["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("再試行する。")
        );
    }

    let target = editor.target(&uri, a);
    assert_eq!(
        editor.execute(3, DELETE_COMMAND, json!([target])),
        Some(Value::Null)
    );
    let shown: Vec<_> = editor
        .lenses(&uri)
        .into_iter()
        .filter(|c| c.command == SHOW_COMMAND)
        .collect();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].arguments.as_ref().unwrap()[0]["range"], json!(b));
    assert_eq!(editor.hover(4, &uri, b.start).unwrap()["range"], json!(b));
    assert!(editor.hover(5, &uri, a.start).is_none());
    assert!(editor.finish_next());
    assert_eq!(editor.reply(5)["range"], json!(a));
}

#[test]
fn deleting_an_exact_range_clears_each_kind_and_the_matching_problem() {
    let uri = "file:///exact.md".parse().unwrap();
    let a = line(0, 11);
    let b = line(1, 12);
    for same_language in [false, true] {
        let mut editor = Editor::new(MockProvider::default(), Settings::default());
        editor.open(&uri, "markdown", "First line.\nSecond line.");
        let target = editor.target(&uri, a);
        let other = editor.target(&uri, b);
        for (ticket, output, range) in [
            (1, Output::Range, a),
            (2, Output::Document, a),
            (3, Output::Range, b),
        ] {
            editor
                .server
                .cache
                .insert(uri.clone(), ticket, translation(output, range));
        }
        let result = if same_language {
            Stored::SameLanguage {
                context: MockContext,
                range: a,
                keys: Vec::new(),
            }
        } else {
            Stored::failure(MockContext, a, "synthetic failure")
        };
        editor.server.cache.insert(uri.clone(), 4, result);
        editor.server.problem = Some(ShownProblem {
            target: target.clone(),
            problem: Problem::Failed("synthetic failure".into()),
            context: Some(MockContext),
        });
        assert!(
            editor
                .server
                .cache
                .exact(&uri, a, Output::Document)
                .is_some()
        );
        assert_eq!(editor.server.cache.same_language(&uri, a), same_language);
        assert_eq!(
            editor.server.cache.failed_range(&uri, a.start).is_some(),
            !same_language
        );

        assert_eq!(
            editor.execute(1, DELETE_COMMAND, json!([target])),
            Some(Value::Null)
        );
        assert_eq!(editor.hover(2, &uri, a.start), Some(Value::Null));
        assert!(
            editor
                .server
                .cache
                .exact(&uri, a, Output::Document)
                .is_none()
        );
        assert!(!editor.server.cache.same_language(&uri, a));
        assert!(editor.server.problem.is_none());
        assert_eq!(editor.hover(3, &uri, b.start).unwrap()["range"], json!(b));

        editor.server.problem = Some(ShownProblem {
            target: other,
            problem: Problem::Failed("other failure".into()),
            context: Some(MockContext),
        });
        editor.execute(4, DELETE_COMMAND, json!([target]));
        assert_eq!(editor.server.problem.as_ref().unwrap().target.range, b);
    }
}

#[test]
fn a_card_deletes_a_current_result_only_when_its_delete_button_is_pressed() {
    let uri = "file:///card.md".parse().unwrap();
    let range = line(0, 9);
    for (button, stale_version, stale_session, fails) in [
        (Value::Null, false, false, false),
        (json!({"title": menu::DELETE}), true, false, false),
        (json!({"title": menu::DELETE}), false, true, false),
        (json!({"title": menu::DELETE}), false, false, false),
        (json!({"title": menu::DELETE}), false, false, true),
    ] {
        let mut editor = Editor::new(MockProvider::default(), Settings::default());
        editor.open(&uri, "markdown", "Sentence.");
        let target = editor.target(&uri, range);
        let result = if fails {
            Stored::failure(MockContext, range, "synthetic failure")
        } else {
            translation(Output::Range, range)
        };
        editor.server.cache.insert(uri.clone(), 1, result);
        editor.execute(1, SHOW_COMMAND, json!([target]));
        if stale_version || stale_session {
            let (version, session) = (1 + i32::from(stale_version), 1 + u64::from(stale_session));
            editor.server.documents.insert(
                uri.clone(),
                Document::new(version, session, "markdown".into(), "New text.".into()),
            );
            editor.server.cache.remove_document(&uri);
            editor
                .server
                .cache
                .insert(uri.clone(), 2, translation(Output::Range, range));
            assert_eq!(
                editor.execute(2, DELETE_COMMAND, json!([target])),
                Some(Value::Null)
            );
            assert_eq!(
                editor.hover(3, &uri, range.start).unwrap()["range"],
                json!(range)
            );
        }
        let deletes = !button.is_null() && !stale_version && !stale_session;
        let prompt = editor.respond_to_prompt(button);
        assert_eq!(
            prompt["actions"],
            json!([{"title": menu::RETRY}, {"title": menu::DELETE}])
        );
        assert_eq!(
            editor.hover(4, &uri, range.start).unwrap().is_null(),
            deletes
        );
    }
}

#[test]
fn settings_passes_its_document_to_delete_only_that_documents_results() {
    let uri = "file:///from-settings.rs".parse().unwrap();
    let other = "file:///other.md".parse().unwrap();
    let a = line(0, 9);
    let b = line(1, 9);
    let settings =
        Settings::from_options(Some(json!({"features": {"comment_lens": false}}))).unwrap();
    let mut editor = Editor::new(MockProvider::default(), settings);
    editor.open(&uri, "rust", "let x = 0;\nlet y = 1;");
    editor.open(&other, "markdown", "Sentence.");
    for (ticket, document, range) in [(1, &uri, a), (2, &uri, b), (3, &other, a)] {
        editor
            .server
            .cache
            .insert(document.clone(), ticket, translation(Output::Range, range));
    }

    assert!(editor.delete_actions(&uri, cursor(0, 2)).is_empty());
    assert_eq!(
        editor.execute(1, MENU_COMMAND, json!([])),
        Some(Value::Null)
    );
    let old_menu = editor.respond_to_prompt(Value::Null);
    assert_eq!(
        old_menu["actions"],
        json!([
            {"title": "Turn off automatic translation"},
            {"title": menu::DELETE_ALL}
        ])
    );

    let settings = editor
        .lenses(&uri)
        .into_iter()
        .find(|c| c.title == "Settings")
        .unwrap();
    assert_eq!(
        editor.execute(2, &settings.command, json!(settings.arguments)),
        Some(Value::Null)
    );
    let menu = editor.respond_to_prompt(json!({"title": menu::DELETE_DOCUMENT}));
    assert_eq!(
        menu["actions"],
        json!([
            {"title": "Turn off automatic translation"},
            {"title": menu::DELETE_DOCUMENT},
            {"title": menu::DELETE_ALL}
        ])
    );
    assert_eq!(
        editor.hover(3, &uri, Position::new(0, 2)),
        Some(Value::Null)
    );
    assert_eq!(
        editor.hover(4, &uri, Position::new(1, 2)),
        Some(Value::Null)
    );
    assert_eq!(editor.hover(5, &other, a.start).unwrap()["range"], json!(a));
}

#[test]
fn delete_all_clears_results_at_confirmation_and_does_not_stop_translation() {
    let uri = "file:///running.rs".parse().unwrap();
    let prose = "file:///ready.md".parse().unwrap();
    let range = comment_range(0);
    let mut editor = Editor::new(MockProvider::default(), Settings::default());
    editor.open(&uri, "rust", COMMENT);
    editor.open(&prose, "markdown", "First comment.");
    editor
        .server
        .cache
        .insert(prose.clone(), 1, translation(Output::Range, line(0, 14)));

    for answer in [Value::Null, json!({"title": "Keep"})] {
        assert_eq!(
            editor.execute(1, DELETE_ALL_COMMAND, json!([])),
            Some(Value::Null)
        );
        editor.respond_to_prompt(answer);
        assert_eq!(
            editor.hover(2, &prose, Position::new(0, 2)).unwrap()["range"],
            json!(line(0, 14))
        );
    }

    assert_eq!(
        editor.execute(3, DELETE_ALL_COMMAND, json!([])),
        Some(Value::Null)
    );
    let prompt = editor.prompt();
    editor
        .server
        .card_response(
            &editor.connection,
            Response::new_ok(
                "untracked-card".to_owned().into(),
                json!({"title": "Delete"}),
            ),
        )
        .unwrap();
    assert_eq!(
        editor.hover(4, &prose, Position::new(0, 2)).unwrap()["range"],
        json!(line(0, 14))
    );
    assert_eq!(editor.delete_actions(&prose, cursor(0, 2)).len(), 3);

    assert_eq!(
        editor.execute(
            5,
            LENS_TRANSLATE_COMMAND,
            json!([editor.target(&uri, range)])
        ),
        Some(Value::Null)
    );
    assert!(editor.finish_next());
    let translated = editor.hover(6, &uri, range.start).unwrap();
    assert_eq!(translated["range"], json!(range));
    assert!(
        translated["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("再試行する。")
    );

    let prompt = editor.answer_prompt(prompt, json!({"title": "Delete"}));
    assert_eq!(
        prompt["message"],
        "Delete all cached translations? Comments and selections are translated again when you ask for them."
    );
    assert_eq!(prompt["actions"], json!([{"title": "Delete"}]));
    assert_eq!(
        editor.hover(7, &prose, Position::new(0, 2)),
        Some(Value::Null)
    );
    assert!(editor.hover(8, &uri, range.start).is_none());
    assert_eq!(
        editor.execute(9, DELETE_COMMAND, json!([editor.target(&uri, range)])),
        Some(Value::Null)
    );
    assert!(editor.finish_next());
    assert_eq!(editor.reply(8)["range"], json!(range));
    assert_eq!(
        editor.hover(10, &uri, range.start).unwrap()["range"],
        json!(range)
    );
}

#[test]
fn a_delete_all_card_from_settings_allows_an_account_menu_to_open() {
    let uri = "file:///settings.rs".parse().unwrap();
    let mut editor = Editor::new(UnsetProvider, Settings::default());
    editor.open(&uri, "rust", COMMENT);
    editor.server.cache.insert(
        uri.clone(),
        1,
        Stored::failure(
            crate::providers::unset::UnsetContext,
            comment_range(0),
            "old failure",
        ),
    );

    assert_eq!(
        editor.execute(1, MENU_COMMAND, json!([uri])),
        Some(Value::Null)
    );
    editor.respond_to_prompt(json!({"title": menu::DELETE_ALL}));
    let confirmation = editor.prompt();
    assert_eq!(confirmation.params["actions"], json!([{"title": "Delete"}]));

    assert_eq!(
        editor.execute(2, SETTINGS_COMMAND, json!([])),
        Some(Value::Null)
    );
    let account = editor.respond_to_prompt(Value::Null);
    assert!(
        account["actions"]
            .as_array()
            .is_some_and(|actions| !actions.is_empty())
    );

    editor.answer_prompt(confirmation, Value::Null);
    assert!(editor.server.cache.has_document(&uri));
}

#[test]
fn delete_actions_follow_prose_selection_and_hover_priority() {
    let uri = "file:///overlap.txt".parse().unwrap();
    let other = "file:///other.md".parse().unwrap();
    let a = Range::new(Position::new(0, 0), Position::new(1, 5));
    let b = Range::new(Position::new(1, 0), Position::new(2, 9));
    for language in ["plaintext", "markdown", "rust"] {
        let mut editor = Editor::new(MockProvider::default(), Settings::default());
        editor.open(&uri, language, "Sentence.\nSentence.\nSentence.");
        editor.open(&other, "markdown", "Sentence.");
        assert!(editor.delete_actions(&uri, cursor(1, 2)).is_empty());
        editor
            .server
            .cache
            .insert(uri.clone(), 1, translation(Output::Range, a));
        editor.server.cache.insert(
            uri.clone(),
            2,
            Stored::failure(MockContext, b, "synthetic failure"),
        );
        assert_eq!(
            editor.hover(1, &uri, Position::new(1, 2)).unwrap()["range"],
            json!(a)
        );
        let at_overlap = editor.delete_actions(&uri, cursor(1, 2));
        if language == "rust" {
            assert!(at_overlap.is_empty());
            continue;
        }
        assert_eq!(
            at_overlap
                .iter()
                .map(|c| c.command.as_str())
                .collect::<Vec<_>>(),
            [DELETE_COMMAND, DELETE_DOCUMENT_COMMAND, DELETE_ALL_COMMAND]
        );
        assert_eq!(at_overlap[0].title, "Delete translation of lines 1-2");
        let at_failure = editor.delete_actions(&uri, cursor(1, 8));
        assert_eq!(at_failure[0].title, "Delete translation of lines 2-3");
        assert_eq!(
            editor
                .delete_actions(&uri, a)
                .iter()
                .map(|c| c.command.as_str())
                .collect::<Vec<_>>(),
            [DELETE_DOCUMENT_COMMAND, DELETE_ALL_COMMAND]
        );
        assert_eq!(
            editor.delete_actions(&other, cursor(0, 0))[0].command,
            DELETE_ALL_COMMAND
        );
    }

    let mut editor = Editor::new(UnsetProvider, Settings::default());
    editor.open(&uri, "plaintext", "Sentence.");
    editor.server.cache.insert(
        uri.clone(),
        1,
        Stored::failure(
            crate::providers::unset::UnsetContext,
            line(0, 9),
            "old failure",
        ),
    );
    assert!(editor.delete_actions(&uri, cursor(0, 3)).is_empty());
}
