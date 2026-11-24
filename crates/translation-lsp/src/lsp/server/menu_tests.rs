//! Menus are cards: opening one answers its command at once, holds no
//! account operation, and the item chosen runs as things are then.
use crossbeam_channel::Receiver;
use serde_json::json;

use super::*;
use crate::{
    lsp::ui::CHOOSE,
    store::{Content, Sent},
};

const WAIT: Duration = Duration::from_secs(5);
const COMMENT: &str = "// Retry only if the remote service\n// reports a temporary failure.";
/// The text of the comment, which the mock provider translates too.
const PROSE: &str = "Retry only if the remote service\nreports a temporary failure.";
const TRANSLATION: &str = "リモートサービスが一時的な障害を報告した場合に限り、再試行する。";
const TURN_OFF: &str = "Turn off automatic translation";
const TURN_ON: &str = "Turn on automatic translation";
const OPEN_SETTINGS: &str = "Open Zed settings file";

struct Editor {
    server: Server<MockProvider>,
    connection: Connection,
    client: Connection,
    events: Receiver<WorkerEvent<MockContext>>,
    uri: Uri,
}

impl Editor {
    /// A server with the comment open, which can open Zed's settings file.
    fn new() -> Self {
        Self::with(UiCapabilities {
            show_document: true,
            ..Default::default()
        })
    }

    /// A server with the comment open, for an editor with `capabilities`.
    fn with(capabilities: UiCapabilities) -> Self {
        let (connection, client) = Connection::memory();
        let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
        let mut server = Server::configured(
            finished,
            MockProvider::default(),
            Settings::default(),
            Ui::new(connection.sender.clone(), capabilities),
        )
        .unwrap();
        let uri: Uri = "file:///menu.rs".parse().unwrap();
        server.documents.insert(
            uri.clone(),
            Document::new(1, 1, "rust".into(), COMMENT.into()),
        );
        Self {
            server,
            connection,
            client,
            events,
            uri,
        }
    }

    /// Runs a command of request `id` as the editor sends it.
    fn request(&mut self, id: i32, command: &str, arguments: Value) {
        self.server
            .request(
                &self.connection,
                Request::new(
                    id.into(),
                    ExecuteCommand::METHOD.into(),
                    json!({"command": command, "arguments": arguments}),
                ),
            )
            .unwrap();
    }

    /// Opens the server menu with request `id`, checks that the request is
    /// answered before anything is chosen, and gives the menu.
    fn open_menu(&mut self, id: i32) -> Request {
        self.open_menu_of(id, &self.uri.clone())
    }

    /// Opens the server menu of the document `uri`, as `open_menu` does.
    fn open_menu_of(&mut self, id: i32, uri: &Uri) -> Request {
        self.request(id, MENU_COMMAND, json!([uri]));
        let Message::Request(menu) = self.next_message() else {
            panic!("Expected the menu");
        };
        assert_eq!(menu.method, "window/showMessageRequest");
        assert!(self.reply(id).unwrap().is_null());
        menu
    }

    fn choose(&mut self, menu: &Request, choice: Value) {
        let Some((Purpose::Card, response)) = self
            .server
            .ui
            .response(Response::new_ok(menu.id.clone(), choice))
        else {
            panic!("Expected the menu to be open");
        };
        self.server
            .card_response(&self.connection, response)
            .unwrap();
    }

    fn next_message(&self) -> Message {
        self.client.receiver.recv_timeout(WAIT).unwrap()
    }

    fn reply(&self, id: i32) -> Result<Value, ResponseError> {
        let Message::Response(response) = self.next_message() else {
            panic!("Expected the response to {id}");
        };
        assert_eq!(response.id, id.into());
        response.response_result
    }

    fn notice(&self) -> Value {
        let Message::Notification(notice) = self.next_message() else {
            panic!("Expected a notice");
        };
        assert_eq!(notice.method, "window/showMessage");
        notice.params
    }

    fn finish_next(&mut self) {
        loop {
            match self.events.recv_timeout(WAIT).unwrap() {
                WorkerEvent::Started(ticket) => self.server.scheduler.mark_running(ticket),
                WorkerEvent::Finished(completion) => {
                    self.server.finish(&self.connection, completion).unwrap();
                    return;
                }
            }
        }
    }

    fn hover(&mut self, id: i32) -> RequestResult<Option<Value>> {
        self.server.hover(
            id.into(),
            parse(json!({
                "textDocument": {"uri": self.uri}, "position": {"line": 0, "character": 4}
            }))
            .unwrap(),
        )
    }

    fn whole_comment(&self) -> Range {
        Range::new(Position::new(0, 0), Position::new(1, 31))
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

    fn assert_quiet(&self) {
        assert!(
            self.client.receiver.is_empty(),
            "{:?}",
            self.client.receiver.try_iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn hover_selection_translation_and_code_actions_work_while_two_menus_are_open() {
    let mut editor = Editor::new();
    let first = editor.open_menu(1);
    let second = editor.open_menu(2);
    assert!(editor.server.administrations.is_empty());
    assert_eq!(editor.server.cards.len(), 2);

    assert_eq!(editor.hover(3).unwrap(), None);
    editor.finish_next();
    let hover = editor.reply(3).unwrap();
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains(TRANSLATION),
        "{hover}"
    );

    let notes: Uri = "file:///notes.txt".parse().unwrap();
    editor.server.documents.insert(
        notes.clone(),
        Document::new(1, 1, "plaintext".into(), PROSE.into()),
    );
    let selection = Range::new(Position::new(0, 0), Position::new(1, 28));
    let target = to_value(editor.target(&notes, selection)).unwrap();
    editor.request(4, COMMAND, json!([target]));
    assert!(editor.reply(4).unwrap().is_null());
    editor.finish_next();
    assert!(
        editor.notice()["message"]
            .as_str()
            .unwrap()
            .ends_with(TRANSLATION_READY)
    );

    let range = editor.whole_comment();
    let actions = editor
        .server
        .code_actions(
            parse(json!({
                "textDocument": {"uri": editor.uri}, "range": range, "context": {"diagnostics": []}
            }))
            .unwrap(),
        )
        .unwrap();
    assert!(
        actions
            .iter()
            .any(|action| action.command.as_ref().unwrap().command == COMMAND)
    );

    // Each menu runs what is chosen on it.
    editor.choose(&second, json!({"title": TURN_OFF}));
    assert!(editor.server.settings.features.automatic_paused());
    assert_eq!(editor.server.cards.len(), 1);
    editor.choose(&first, Value::Null);
    assert!(editor.server.cards.is_empty());
    editor.assert_quiet();
}

#[test]
fn the_item_chosen_runs_as_the_command_would_run_when_it_is_chosen() {
    let mut editor = Editor::new();

    // The menu was opened while automatic translation was on. It was turned
    // off and on again elsewhere, and a comment Hover is on its way. Turning
    // it off from the menu cancels that Hover, as the command would now.
    let menu = editor.open_menu(1);
    editor.request(2, AUTOMATIC_OFF_COMMAND, json!([]));
    assert!(editor.reply(2).unwrap().is_null());
    editor.request(3, AUTOMATIC_ON_COMMAND, json!([]));
    assert!(editor.reply(3).unwrap().is_null());
    assert_eq!(editor.hover(4).unwrap(), None);
    editor.choose(&menu, json!({"title": TURN_OFF}));
    assert!(editor.server.settings.features.automatic_paused());
    assert_eq!(
        editor.reply(4).unwrap_err().code,
        ErrorCode::RequestCanceled as i32
    );

    // The menu was opened with the document's results, and the document
    // was closed before its item was chosen. Deleting its translations then
    // does what the command does for a closed document: it refreshes the
    // lines, keeps the translation stored for the text the document had,
    // which it no longer knows, and keeps the results of another document.
    editor.server.ui.capabilities.lens_refresh = true;
    let stored = editor.server.store.key(
        Output::Range,
        &editor.server.settings.target_language,
        Sent::Passage(COMMENT),
    );
    editor
        .server
        .store
        .put(stored, Content::Translation("訳".into()));
    let other: Uri = "file:///other.md".parse().unwrap();
    editor.server.documents.insert(
        other.clone(),
        Document::new(1, 1, "markdown".into(), "Sentence.".into()),
    );
    let sentence = Range::new(Position::new(0, 0), Position::new(0, 9));
    for (ticket, uri, range) in [
        (1, &editor.uri.clone(), editor.whole_comment()),
        (2, &other, sentence),
    ] {
        editor.server.cache.insert(
            uri.clone(),
            ticket,
            Stored::Translation(
                Output::Range,
                Preview::new(MockContext, range, "訳".into(), false, None),
            ),
        );
    }
    let menu = editor.open_menu(5);
    assert_eq!(
        menu.params["actions"][2]["title"],
        "Delete translations of this document"
    );
    let uri = editor.uri.clone();
    editor
        .server
        .notification(
            &editor.connection,
            lsp_server::Notification::new(
                DidCloseTextDocument::METHOD.into(),
                json!({"textDocument": {"uri": uri}}),
            ),
        )
        .unwrap();
    editor.choose(
        &menu,
        json!({"title": "Delete translations of this document"}),
    );
    let Message::Request(refresh) = editor.next_message() else {
        panic!("Expected the lines to be refreshed");
    };
    assert_eq!(refresh.method, "workspace/codeLens/refresh");
    assert!(!editor.server.cache.has_document(&uri));
    assert!(editor.server.cache.has_document(&other));
    assert!(editor.server.store.peek(stored).is_some());
    editor.assert_quiet();
}

#[test]
fn each_menu_deletes_the_translations_of_its_own_document() {
    let mut editor = Editor::new();
    let first = editor.uri.clone();
    let second: Uri = "file:///second.rs".parse().unwrap();
    editor.server.documents.insert(
        second.clone(),
        Document::new(1, 1, "rust".into(), COMMENT.into()),
    );
    for (ticket, uri) in [(1, &first), (2, &second)] {
        editor.server.cache.insert(
            uri.clone(),
            ticket,
            Stored::Translation(
                Output::Range,
                Preview::new(
                    MockContext,
                    editor.whole_comment(),
                    "訳".into(),
                    false,
                    None,
                ),
            ),
        );
    }

    // Both menus offer the same titles. The one opened later deletes only
    // the translations of its document, and the other still works.
    let first_menu = editor.open_menu_of(1, &first);
    let second_menu = editor.open_menu_of(2, &second);
    assert_eq!(first_menu.params["actions"], second_menu.params["actions"]);
    editor.choose(
        &second_menu,
        json!({"title": "Delete translations of this document"}),
    );
    assert!(!editor.server.cache.has_document(&second));
    assert!(editor.server.cache.has_document(&first));
    assert!(editor.server.cards.contains_key(&first_menu.id));
    editor.choose(
        &first_menu,
        json!({"title": "Delete translations of this document"}),
    );
    assert!(!editor.server.cache.has_document(&first));
    assert!(editor.server.cards.is_empty());
    editor.assert_quiet();
}

#[test]
fn a_menu_opens_during_an_account_operation_and_refuses_only_account_operations() {
    let mut editor = Editor::new();
    editor.request(1, OPEN_SETTINGS_COMMAND, json!([]));
    let Message::Request(opening) = editor.next_message() else {
        panic!("Expected the settings file to open");
    };
    assert!(editor.server.administration_of(&1.into()).is_some());

    // Menus opened after the operation started open, and an item that is
    // no account operation runs.
    let switch = editor.open_menu(2);
    let operation = editor.open_menu(3);
    editor.choose(&switch, json!({"title": TURN_OFF}));
    assert!(editor.server.settings.features.automatic_paused());
    editor.assert_quiet();

    // An item that starts an account operation is refused with a notice.
    editor.choose(&operation, json!({"title": OPEN_SETTINGS}));
    assert_eq!(
        editor.notice()["message"],
        "An account operation is in progress"
    );
    editor.assert_quiet();
    assert!(editor.server.administration_of(&1.into()).is_some());

    editor
        .server
        .ui
        .response(Response::new_ok(opening.id, json!({"success": true})));
    editor.finish_next();
    assert!(editor.reply(1).unwrap().is_null());
    assert!(editor.server.administrations.is_empty());
    editor.assert_quiet();
}

#[test]
fn closing_a_menu_or_an_unknown_response_does_nothing() {
    let mut editor = Editor::new();
    for answer in [
        Value::Null,
        json!({"title": "Not on the menu"}),
        json!({"other": TURN_OFF}),
    ] {
        let menu = editor.open_menu(1);
        editor.choose(&menu, answer);
        assert!(editor.server.cards.is_empty());
        assert!(!editor.server.settings.features.automatic_paused());
        assert!(editor.server.administrations.is_empty());
        editor.assert_quiet();
    }

    // A response to a request the server does not remember, such as one to
    // a menu that was answered already, is dropped.
    let menu = editor.open_menu(2);
    editor.choose(&menu, json!({"title": TURN_OFF}));
    assert!(editor.server.settings.features.automatic_paused());
    assert!(
        editor
            .server
            .ui
            .response(Response::new_ok(menu.id, json!({"title": TURN_ON})))
            .is_none()
    );
    editor
        .server
        .card_response(
            &editor.connection,
            Response::new_ok("unknown-card".to_owned().into(), json!({"title": TURN_ON})),
        )
        .unwrap();
    assert!(editor.server.settings.features.automatic_paused());
    editor.assert_quiet();
}

#[test]
fn a_menu_has_no_deadline_and_its_account_operation_answers_no_request() {
    let mut editor = Editor::new();
    let menu = editor.open_menu(1);

    // An account operation that times out leaves the menu open.
    editor.request(2, OPEN_SETTINGS_COMMAND, json!([]));
    let Message::Request(opening) = editor.next_message() else {
        panic!("Expected the settings file to open");
    };
    assert_eq!(opening.method, "window/showDocument");
    let ticket = editor.server.administration_of(&2.into()).unwrap();
    editor
        .server
        .administrations
        .get_mut(&ticket)
        .unwrap()
        .deadline = Instant::now();
    editor.server.expire(&editor.connection).unwrap();
    editor.finish_next();
    // The worker withdraws the opening while the server reports the
    // timeout, so the withdrawal comes at any place among the messages.
    let mut messages: Vec<_> = (0..3).map(|_| editor.next_message()).collect();
    let withdrawal = messages
        .iter()
        .position(|message| {
            matches!(message, Message::Notification(notice) if notice.method == "$/cancelRequest")
        })
        .expect("Expected the opening to be withdrawn");
    messages.remove(withdrawal);
    let [Message::Notification(notice), Message::Response(response)] = &messages[..] else {
        panic!("Expected the notice and the response: {messages:?}");
    };
    assert_eq!(notice.method, "window/showMessage");
    assert_eq!(notice.params["message"], "Account operation timed out");
    assert_eq!(response.id, 2.into());
    assert_eq!(
        response.response_result.as_ref().unwrap_err().message,
        "Account operation timed out"
    );
    assert!(editor.server.cards.contains_key(&menu.id));

    // The operation chosen on the menu starts its own deadline when it is
    // chosen, and answers no request when it ends.
    let chosen = Instant::now();
    editor.choose(&menu, json!({"title": OPEN_SETTINGS}));
    let [work] = editor
        .server
        .administrations
        .values()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("Expected one account operation"));
    assert!(work.request.is_none());
    assert!(work.deadline >= chosen + Duration::from_secs(179));
    let Message::Request(opening) = editor.next_message() else {
        panic!("Expected the settings file to open");
    };
    editor
        .server
        .ui
        .response(Response::new_ok(opening.id, json!({"success": false})));
    editor.finish_next();
    assert!(editor.server.administrations.is_empty());
    let notice = editor.notice();
    assert_eq!(notice["type"], 1);
    assert!(
        notice["message"]
            .as_str()
            .unwrap()
            .starts_with("Could not open Zed's settings file"),
        "{notice}"
    );
    editor.assert_quiet();

    // An operation chosen while another runs is refused as the command
    // would be, with a notice and no response.
    let menu = editor.open_menu(3);
    editor.request(4, OPEN_SETTINGS_COMMAND, json!([]));
    let Message::Request(opening) = editor.next_message() else {
        panic!("Expected the settings file to open");
    };
    editor.choose(&menu, json!({"title": OPEN_SETTINGS}));
    assert_eq!(
        editor.notice()["message"],
        "An account operation is in progress"
    );
    editor.assert_quiet();
    editor
        .server
        .ui
        .response(Response::new_ok(opening.id, json!({"success": true})));
    editor.finish_next();
    assert!(editor.reply(4).unwrap().is_null());
    editor.assert_quiet();
}

/// A client that names `choose` gets the menu on one line, each choice with
/// an ID, and the ID it answers picks the choice even where two choices have
/// the same title. `null`, an ID that names no choice, and an answer of
/// another shape do nothing.
#[test]
fn a_client_that_names_choose_picks_a_menu_choice_by_its_id() {
    let mut editor = Editor::with(UiCapabilities {
        choose: true,
        show_document: true,
        ..Default::default()
    });
    editor.request(1, MENU_COMMAND, json!([editor.uri]));
    let Message::Request(menu) = editor.next_message() else {
        panic!("Expected the menu");
    };
    assert!(editor.reply(1).unwrap().is_null());
    assert_eq!(menu.method, CHOOSE);
    assert_eq!(
        menu.params,
        json!({
            "kind": "menu",
            "message": "Automatic translation: On (comment Hover and automatic selection translation)",
            "choices": [
                {"id": "0", "title": TURN_OFF},
                {"id": "1", "title": OPEN_SETTINGS}
            ]
        })
    );
    editor.choose(&menu, Value::Null);
    for answer in [
        json!({"id": "2"}),
        json!({"id": "+0"}),
        json!({"id": "00"}),
        json!({"id": 0}),
        json!({"title": TURN_OFF}),
    ] {
        let choices = vec![
            ServerCommand::Automatic(false)
                .command(TURN_OFF.into())
                .unwrap(),
        ];
        editor.server.open_menu("", choices).unwrap();
        let Message::Request(menu) = editor.next_message() else {
            panic!("Expected the menu");
        };
        editor.choose(&menu, answer.clone());
        assert!(editor.server.cards.is_empty(), "{answer}");
        assert!(
            !editor.server.settings.features.automatic_paused(),
            "{answer}"
        );
    }

    // Two choices with one title: the second turns automatic translation off.
    let same = "Automatic translation";
    let choices = vec![
        ServerCommand::Automatic(true).command(same.into()).unwrap(),
        ServerCommand::Automatic(false)
            .command(same.into())
            .unwrap(),
    ];
    editor.server.open_menu("", choices).unwrap();
    let Message::Request(menu) = editor.next_message() else {
        panic!("Expected the menu");
    };
    assert_eq!(
        menu.params["choices"],
        json!([{"id": "0", "title": same}, {"id": "1", "title": same}])
    );
    editor.choose(&menu, json!({"id": "1"}));
    assert!(editor.server.settings.features.automatic_paused());
    assert!(editor.server.cards.is_empty());
}

/// The question before every translation is deleted is a confirmation for a
/// client that names `choose`, and a message with a button for any other.
#[test]
fn deleting_all_translations_is_confirmed_as_the_client_shows_it() {
    for choose in [false, true] {
        let mut editor = Editor::with(UiCapabilities {
            choose,
            ..Default::default()
        });
        editor.server.confirm_delete_all().unwrap();
        let Message::Request(question) = editor.next_message() else {
            panic!("Expected the question");
        };
        let message = "Delete all cached translations? Comments and selections are translated again when you ask for them.";
        if choose {
            assert_eq!(question.method, CHOOSE);
            assert_eq!(
                question.params,
                json!({
                    "kind": "confirm",
                    "message": message,
                    "choices": [{"id": "0", "title": "Delete"}]
                })
            );
        } else {
            assert_eq!(question.method, "window/showMessageRequest");
            assert_eq!(
                question.params,
                json!({"type": 3, "message": message, "actions": [{"title": "Delete"}]})
            );
        }
    }
}
