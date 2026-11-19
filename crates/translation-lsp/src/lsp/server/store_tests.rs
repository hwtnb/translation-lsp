//! Translations kept by what was sent: in memory, and on disk with
//! `disk_cache`.

use std::{
    env, fs,
    sync::{Arc, Mutex, MutexGuard},
};

use crossbeam_channel::Receiver;
use serde_json::json;
use tokio::time::sleep;
use uuid::Uuid;

use super::{menu::DELETE_ALL, *};
use crate::{
    lsp::preview::Form,
    providers::{Intent, Kind, mock::MockContext},
    store::Identity,
};

const WAIT: Duration = Duration::from_secs(5);
/// A Rust file with the same comment twice.
const CODE: &str = "// Alpha comment.\nfn a() {}\n// Alpha comment.\nfn b() {}\n";

/// What the provider of these tests was asked.
#[derive(Default)]
struct Record {
    /// The texts sent, whole, in parts or as terms, in order.
    sent: Vec<String>,
    /// Automatic requests find their text in the target language.
    same_language: bool,
    /// A request for a text that contains this fails, after the others.
    fail: Option<String>,
    /// Added to the time each request takes.
    delay: Duration,
    /// Written after each translation, to tell one request from the next.
    suffix: String,
}

#[derive(Clone, Default)]
struct Counter(Arc<Mutex<Record>>);

impl Counter {
    fn record(&self) -> MutexGuard<'_, Record> {
        self.0.lock().unwrap()
    }

    fn sent(&self) -> Vec<String> {
        self.record().sent.clone()
    }

    async fn answer(&self, text: &str) -> Result<String, ProviderError> {
        let delay = {
            let mut record = self.record();
            record.sent.push(text.to_owned());
            record.delay
        };
        sleep(delay).await;
        let fails = self
            .record()
            .fail
            .as_ref()
            .is_some_and(|fail| text.contains(fail.as_str()));
        if fails {
            sleep(Duration::from_millis(300)).await;
            return Err(ProviderError::failed("Synthetic failure"));
        }
        Ok(format!("訳: {text}{}", self.record().suffix))
    }
}

impl TranslationProvider for Counter {
    type Context = MockContext;

    fn context(&self) -> Result<MockContext, ProviderError> {
        Ok(MockContext)
    }

    fn identity(&self, _output: Output) -> Identity {
        Identity::new("counter")
    }

    async fn translate(
        &self,
        intent: Intent,
        _output: Output,
        text: &str,
        _target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, ProviderError> {
        let text = self.answer(text).await?;
        if intent == Intent::Automatic && self.record().same_language {
            return Ok(Translated {
                text: String::new(),
                context,
                kind: Kind::SameLanguage,
            });
        }
        Ok(Translated {
            text,
            context,
            kind: Kind::Translation,
        })
    }

    async fn translate_part(
        &self,
        text: &str,
        _target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, ProviderError> {
        Ok(Translated {
            text: self.answer(text).await?,
            context,
            kind: Kind::Translation,
        })
    }

    async fn look_up(
        &self,
        _intent: Intent,
        _output: Output,
        term: &str,
        line: &str,
        _target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, ProviderError> {
        let text = self.answer(&format!("{term} | {line}")).await?;
        Ok(Translated {
            text,
            context,
            kind: Kind::Entry(format!("{term} — 意味")),
        })
    }
}

/// A directory of a test's own for stored translations, removed when the
/// test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = env::temp_dir().join(format!("translation-lsp-server-store-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn translations(&self) -> PathBuf {
        self.0.join("translations")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Editor {
    server: Server<Counter>,
    connection: Connection,
    client: Connection,
    events: Receiver<WorkerEvent<MockContext>>,
    next_id: i32,
    next_session: u64,
}

impl Editor {
    fn new(provider: Counter, settings: Value, directory: Option<PathBuf>) -> Self {
        let (connection, client) = Connection::memory();
        let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
        let server = Server::start(
            finished,
            provider,
            Settings::from_options(Some(settings)).unwrap(),
            Ui::new(
                connection.sender.clone(),
                UiCapabilities {
                    show_document: true,
                    ..Default::default()
                },
            ),
            directory,
        )
        .unwrap();
        Self {
            server,
            connection,
            client,
            events,
            next_id: 0,
            next_session: 0,
        }
    }

    fn open(&mut self, name: &str, language: &str, text: &str) -> Uri {
        let uri: Uri = format!("file:///{name}").parse().unwrap();
        self.next_session += 1;
        self.server.documents.insert(
            uri.clone(),
            Document::new(1, self.next_session, language.into(), text.into()),
        );
        uri
    }

    /// Replaces the text of the document, as an edit does.
    fn edit(&mut self, uri: &Uri, text: &str) {
        let version = self.server.documents[uri].version + 1;
        self.server
            .notification(
                &self.connection,
                lsp_server::Notification::new(
                    DidChangeTextDocument::METHOD.into(),
                    json!({
                        "textDocument": {"uri": uri, "version": version},
                        "contentChanges": [{"text": text}]
                    }),
                ),
            )
            .unwrap();
    }

    fn target(&self, uri: &Uri, range: Range) -> TranslationTarget {
        let document = &self.server.documents[uri];
        TranslationTarget {
            uri: uri.clone(),
            version: document.version,
            session: document.session,
            range,
            target_language: "ja".into(),
        }
    }

    fn id(&mut self) -> RequestId {
        self.next_id += 1;
        self.next_id.into()
    }

    /// Ends the next job that the worker finishes.
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

    /// The Hover over `position` once its translation completes.
    fn hover(&mut self, uri: &Uri, position: Position) -> Value {
        let id = self.id();
        let params = parse(json!({"textDocument": {"uri": uri}, "position": position})).unwrap();
        if let Some(value) = self.server.hover(id.clone(), params).unwrap() {
            return value;
        }
        self.finish_next();
        self.reply(&id)
    }

    fn reply(&mut self, id: &RequestId) -> Value {
        loop {
            if let Message::Response(response) = self.client.receiver.recv_timeout(WAIT).unwrap()
                && &response.id == id
            {
                return response.response_result.unwrap();
            }
        }
    }

    /// Runs a translation of `range` that waits for nothing but its result.
    fn translate(&mut self, uri: &Uri, range: Range, reply: PendingReply) {
        let id = self.id();
        let target = self.target(uri, range);
        self.server.queue(id, target, reply).unwrap();
        self.finish_next();
    }

    fn tab(&mut self, uri: &Uri) {
        let range = self.server.documents[uri].full_range().unwrap();
        let target = self.target(uri, range);
        self.server
            .queue_translation(target, TranslationRequest::Document)
            .unwrap();
        self.finish_next();
    }

    fn execute(&mut self, command: &str, arguments: Value) {
        let id = self.id();
        self.server
            .execute_command(
                &self.connection,
                id,
                parse(json!({"command": command, "arguments": arguments})).unwrap(),
            )
            .unwrap();
    }

    fn delete_all(&mut self) {
        self.execute(DELETE_ALL_COMMAND, json!([]));
        let card = loop {
            if let Message::Request(request) = self.client.receiver.recv_timeout(WAIT).unwrap()
                && request.method == "window/showMessageRequest"
            {
                break request;
            }
        };
        if let Some((Purpose::Card, response)) = self
            .server
            .ui
            .response(Response::new_ok(card.id, json!({"title": "Delete"})))
        {
            self.server.card_response(response).unwrap();
        }
    }

    /// The titles of the Code Actions that delete translations at `cursor`.
    fn delete_titles(&mut self, uri: &Uri, cursor: Position) -> Vec<String> {
        self.server
            .code_actions(
                parse(json!({
                    "textDocument": {"uri": uri},
                    "range": Range::new(cursor, cursor),
                    "context": {"diagnostics": []}
                }))
                .unwrap(),
            )
            .unwrap()
            .into_iter()
            .filter_map(|action| action.command)
            .filter(|command| command.command.starts_with("translation-lsp.delete-"))
            .map(|command| command.title)
            .collect()
    }
}

/// Where Hover reads the first comment of [`CODE`], and the second.
const FIRST: Position = Position {
    line: 0,
    character: 4,
};
const SECOND: Position = Position {
    line: 2,
    character: 4,
};

fn shown(hover: &Value) -> &str {
    hover["contents"]["value"].as_str().unwrap_or_default()
}

impl Editor {
    /// The translation that Hover over the comment on `line` of [`CODE`]
    /// shows.
    fn comment(&mut self, uri: &Uri, line_number: u32) -> String {
        let hover = self.hover(uri, Position::new(line_number, 4));
        assert!(!shown(&hover).is_empty(), "{hover}");
        let text = self.server.documents[uri].text().unwrap();
        let length = text.lines().nth(line_number as usize).unwrap().len() as u32;
        self.server
            .cache
            .exact(uri, line(line_number, length), Output::Range)
            .unwrap_or_else(|| panic!("line {line_number}: {hover}"))
            .translation
    }
}

fn line(line: u32, length: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, length))
}

#[test]
fn the_same_comment_is_sent_once_wherever_it_is_and_after_edits() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    assert_eq!(editor.comment(&uri, 0), "訳: Alpha comment.");
    assert_eq!(editor.comment(&uri, 2), "訳: Alpha comment.");
    assert_eq!(provider.sent(), ["Alpha comment."]);
    // An edit elsewhere leaves the comment's text as it was.
    editor.edit(&uri, &format!("// Beta comment.\nfn z() {{}}\n{CODE}"));
    assert_eq!(editor.server.cache.lenses(&uri).len(), 0);
    assert_eq!(editor.comment(&uri, 2), "訳: Alpha comment.");
    let other = editor.open("b.rs", "rust", CODE);
    assert_eq!(editor.comment(&other, 0), "訳: Alpha comment.");
    assert_eq!(provider.sent(), ["Alpha comment."]);
    // The new comment is sent.
    assert_eq!(editor.comment(&uri, 0), "訳: Beta comment.");
    assert_eq!(provider.sent(), ["Alpha comment.", "Beta comment."]);
}

#[test]
fn only_automatic_requests_use_a_stored_same_language_result() {
    let provider = Counter::default();
    provider.record().same_language = true;
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    assert_eq!(editor.hover(&uri, FIRST), Value::Null);
    assert_eq!(editor.hover(&uri, SECOND), Value::Null);
    assert_eq!(provider.sent().len(), 1);
    // An explicit request always translates.
    let comment = line(2, 17);
    editor.translate(&uri, comment, PendingReply::Command { notice: false });
    assert_eq!(provider.sent().len(), 2);
    assert!(
        editor
            .server
            .cache
            .exact(&uri, comment, Output::Range)
            .is_some()
    );
    // Its translation then answers automatic requests too.
    let other = editor.open("b.rs", "rust", CODE);
    assert_eq!(editor.comment(&other, 0), "訳: Alpha comment.");
    assert_eq!(provider.sent().len(), 2);
}

#[test]
fn a_dictionary_entry_is_stored_with_its_line() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    for name in ["a.md", "b.md"] {
        let uri = editor.open(name, "markdown", "Keep retain here.");
        let term = Range::new(Position::new(0, 5), Position::new(0, 11));
        editor.translate(&uri, term, PendingReply::Command { notice: false });
        let preview = editor
            .server
            .cache
            .exact(&uri, term, Output::Range)
            .unwrap();
        assert!(matches!(&preview.form, Form::Entry(line) if line == "retain — 意味"));
    }
    assert_eq!(provider.sent(), ["retain | Keep retain here."]);
}

/// A Markdown document of `count` sections, each sent as a part with
/// `chunk_kib` 1.
fn sections(count: usize, changed: Option<usize>) -> String {
    (1..=count)
        .map(|section| {
            let word = if changed == Some(section) {
                "Changed"
            } else {
                "Sentence"
            };
            format!(
                "# Section {section}\n\n{}",
                format!("{word} {section} of the document. ").repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn split_settings() -> Value {
    json!({"document_split": {"enabled": true, "parallel": 2, "chunk_kib": 1}})
}

#[test]
fn a_tab_sends_only_the_parts_that_changed_or_failed() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), split_settings(), None);
    let uri = editor.open("long.md", "markdown", &sections(4, None));
    editor.tab(&uri);
    assert_eq!(provider.sent().len(), 4);
    editor.edit(&uri, &sections(4, Some(3)));
    editor.tab(&uri);
    let sent = provider.sent();
    assert_eq!(sent.len(), 5);
    assert!(sent[4].starts_with("# Section 3\n\nChanged"));
    let tab = editor
        .server
        .cache
        .exact(
            &uri,
            editor.server.documents[&uri].full_range().unwrap(),
            Output::Document,
        )
        .unwrap();
    assert_eq!(tab.keys.len(), 4);
    assert!(tab.translation.contains("訳: # Section 3\n\nChanged"));

    // A failed part ends the tab, but the parts that completed are kept.
    let edited = sections(4, Some(1)).replace("Sentence 4", "Changed 4");
    editor.edit(&uri, &edited);
    provider.record().fail = Some("Changed 1".into());
    editor.tab(&uri);
    assert!(!editor.server.cache.has_document(&uri));
    let sent = provider.sent();
    assert_eq!(sent.len(), 7, "{sent:?}");
    provider.record().fail = None;
    editor.tab(&uri);
    let sent = provider.sent();
    assert_eq!(sent.len(), 8, "{sent:?}");
    assert!(sent[7].contains("Changed 1"));
    assert!(editor.server.cache.has_document(&uri));
}

#[test]
fn a_retry_sends_and_its_result_takes_the_place_of_the_stored_one() {
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    assert_eq!(editor.comment(&uri, 0), "訳: Alpha comment.");
    provider.record().suffix = " (2)".into();
    editor.translate(&uri, line(0, 17), PendingReply::Retry { notice: false });
    assert_eq!(provider.sent().len(), 2);
    let other = editor.open("b.rs", "rust", CODE);
    assert_eq!(editor.comment(&other, 0), "訳: Alpha comment. (2)");
    assert_eq!(provider.sent().len(), 2);
}

#[test]
fn failed_and_canceled_translations_are_not_stored() {
    let provider = Counter::default();
    provider.record().fail = Some("Alpha".into());
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    editor.translate(&uri, line(0, 17), PendingReply::Command { notice: false });
    assert!(editor.server.store.is_empty());
    provider.record().fail = None;
    provider.record().delay = Duration::from_secs(1);
    let id = editor.id();
    let target = editor.target(&uri, line(0, 17));
    editor
        .server
        .queue(id, target, PendingReply::Command { notice: false })
        .unwrap();
    while provider.sent().len() < 2 {
        std::thread::sleep(Duration::from_millis(5));
    }
    editor.execute(CANCEL_ALL_COMMAND, json!([]));
    editor.finish_next();
    std::thread::sleep(Duration::from_millis(1200));
    assert!(editor.server.store.is_empty());
}

#[test]
fn timed_out_and_canceled_translations_are_kept_nowhere() {
    for cancel in [false, true] {
        let scratch = Scratch::new();
        let provider = Counter::default();
        provider.record().delay = Duration::from_millis(1500);
        let mut editor = Editor::new(
            provider.clone(),
            json!({"disk_cache": {"enabled": true}, "selection_timeout_ms": 1000}),
            Some(scratch.translations()),
        );
        let uri = editor.open("a.rs", "rust", CODE);
        let id = editor.id();
        let target = editor.target(&uri, line(0, 17));
        editor
            .server
            .queue(id, target, PendingReply::Command { notice: false })
            .unwrap();
        while provider.sent().is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
        if cancel {
            editor.execute(CANCEL_ALL_COMMAND, json!([]));
        }
        editor.finish_next();
        // Past when the provider would have answered.
        std::thread::sleep(Duration::from_millis(800));
        assert!(editor.server.store.is_empty(), "cancel {cancel}");
        drop(editor);
        let stored = fs::read_dir(scratch.translations())
            .map(|entries| entries.count())
            .unwrap_or_default();
        assert_eq!(stored, 0, "cancel {cancel}");
    }
}

#[test]
fn deleting_an_explicit_translation_forgets_that_the_text_was_in_the_target_language() {
    let provider = Counter::default();
    provider.record().same_language = true;
    let mut editor = Editor::new(provider.clone(), json!({}), None);
    let uri = editor.open("a.rs", "rust", CODE);
    assert_eq!(editor.hover(&uri, FIRST), Value::Null);
    editor.translate(&uri, line(0, 17), PendingReply::Command { notice: false });
    assert_eq!(provider.sent().len(), 2);
    delete(&mut editor, &uri, "range");
    editor.hover(&uri, FIRST);
    assert_eq!(provider.sent().len(), 3);
}

/// Deletes the translations of the first comment of [`CODE`] in `uri` in the
/// way `how` names.
fn delete(editor: &mut Editor, uri: &Uri, how: &str) {
    match how {
        "range" => {
            let target = editor.target(uri, line(0, 17));
            editor.execute(DELETE_COMMAND, json!([target]));
        }
        "document" => editor.execute(DELETE_DOCUMENT_COMMAND, json!([uri])),
        _ => editor.delete_all(),
    }
}

#[test]
fn after_a_deletion_the_next_hover_sends_the_text_again() {
    for disk in [false, true] {
        for how in ["range", "document", "all"] {
            let scratch = Scratch::new();
            let provider = Counter::default();
            let settings = json!({"disk_cache": {"enabled": disk}});
            let mut editor = Editor::new(
                provider.clone(),
                settings.clone(),
                Some(scratch.translations()),
            );
            let uri = editor.open("a.rs", "rust", CODE);
            editor.hover(&uri, FIRST);
            assert_eq!(provider.sent().len(), 1);
            delete(&mut editor, &uri, how);
            editor.hover(&uri, FIRST);
            assert_eq!(provider.sent().len(), 2, "{how}, disk {disk}");
            if !disk {
                assert!(!scratch.translations().exists());
                continue;
            }
            // A restarted server finds the translation on disk, and deleting
            // the document's translations removes what its text sends, though
            // nothing of it is in the cache.
            drop(editor);
            let mut editor = Editor::new(provider.clone(), settings, Some(scratch.translations()));
            let uri = editor.open("a.rs", "rust", CODE);
            editor.hover(&uri, SECOND);
            assert_eq!(provider.sent().len(), 2, "{how}");
            drop(editor);
            let mut editor = Editor::new(
                provider.clone(),
                json!({"disk_cache": {"enabled": true}}),
                Some(scratch.translations()),
            );
            let uri = editor.open("a.rs", "rust", CODE);
            delete(&mut editor, &uri, "document");
            editor.hover(&uri, FIRST);
            assert_eq!(provider.sent().len(), 3, "{how}");
        }
    }
}

#[test]
fn deleting_a_split_tab_removes_its_parts_after_a_restart() {
    let scratch = Scratch::new();
    let provider = Counter::default();
    let mut settings = split_settings();
    settings["disk_cache"] = json!({"enabled": true});
    let mut editor = Editor::new(
        provider.clone(),
        settings.clone(),
        Some(scratch.translations()),
    );
    let uri = editor.open("long.md", "markdown", &sections(3, None));
    editor.tab(&uri);
    assert_eq!(provider.sent().len(), 3);
    drop(editor);
    let mut editor = Editor::new(provider.clone(), settings, Some(scratch.translations()));
    let uri = editor.open("long.md", "markdown", &sections(3, None));
    editor.execute(DELETE_DOCUMENT_COMMAND, json!([uri]));
    editor.tab(&uri);
    assert_eq!(provider.sent().len(), 6);
}

#[test]
fn deletion_is_offered_while_translations_are_stored() {
    let scratch = Scratch::new();
    let provider = Counter::default();
    let mut editor = Editor::new(provider.clone(), json!({}), Some(scratch.translations()));
    let uri = editor.open("a.md", "markdown", "First line.\n");
    let cursor = Position::new(0, 3);
    assert!(editor.delete_titles(&uri, cursor).is_empty());
    editor.translate(&uri, line(0, 11), PendingReply::Command { notice: false });
    // An edit empties the cache of the document, but not the store.
    editor.edit(&uri, "First line.\nSecond line.\n");
    assert!(!editor.server.cache.has_document(&uri));
    let titles = [menu::DELETE_DOCUMENT, DELETE_ALL];
    assert_eq!(editor.delete_titles(&uri, cursor), titles);
    let menu: Vec<_> = editor
        .server
        .menu_choices(Some(&uri))
        .unwrap()
        .into_iter()
        .map(|command| command.title)
        .collect();
    assert!(
        titles
            .iter()
            .all(|title| menu.contains(&(*title).to_owned()))
    );
    editor.delete_all();
    assert!(editor.delete_titles(&uri, cursor).is_empty());

    // With `disk_cache`, the disk may keep translations of another run.
    let mut editor = Editor::new(
        provider,
        json!({"disk_cache": {"enabled": true}}),
        Some(scratch.translations()),
    );
    let uri = editor.open("a.md", "markdown", "First line.\n");
    assert_eq!(editor.delete_titles(&uri, cursor), titles);
}

/// The check of `docs/development.md` with `samples/long-document.md`.
#[test]
fn deleting_a_list_item_of_the_long_sample_sends_one_part_again() {
    let sample = include_str!("../../../../../samples/long-document.md").replace("\r\n", "\n");
    let provider = Counter::default();
    let mut editor = Editor::new(
        provider.clone(),
        json!({"document_split": {"enabled": true, "chunk_kib": 1}}),
        None,
    );
    let uri = editor.open("long-document.md", "markdown", &sample);
    editor.tab(&uri);
    assert_eq!(provider.sent().len(), 4);
    let item = "- A request on the connection failed with a network error.\n";
    assert!(sample.contains(item));
    editor.edit(&uri, &sample.replace(item, ""));
    editor.tab(&uri);
    let sent = provider.sent();
    assert_eq!(sent.len(), 5, "{sent:#?}");
    assert!(sent[4].contains("- The server closed its end of the connection."));
}
