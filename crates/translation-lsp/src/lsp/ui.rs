#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use std::env;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crossbeam_channel::Sender;
use lsp_server::{Message, Notification, Request, RequestId, Response};
use lsp_types::{ClientCapabilities, MessageType};
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::oneshot;
use uuid::Uuid;

use super::preview::Markup;
use crate::providers::{ChoiceKind, Interaction, error::Error};

/// The notification that hands a translation to a client that names
/// `showTranslation`.
pub(crate) const SHOW_TRANSLATION: &str = "translation-lsp/showTranslation";
/// The request that asks a client that names `openSettings` to open its
/// settings.
pub(crate) const OPEN_SETTINGS: &str = "translation-lsp/openSettings";
/// The request that asks a client that names `choose` to show a menu or a
/// confirmation of its own, in place of a message with buttons.
pub(crate) const CHOOSE: &str = "translation-lsp/choose";
/// What a client that opens its own settings is told when it could not.
pub(crate) const SETTINGS_NOT_OPENED: &str = "Could not open the settings";
pub(crate) const NO_SETTINGS_FILE: &str =
    "Could not open Zed's settings file. Run zed: open settings file from the Command Palette";

/// Every request to the editor, whether a task awaits its response or the
/// server loop handles it, is registered in one [`Ui`].
#[derive(Clone)]
pub(crate) struct Ui {
    pub(super) sender: Sender<Message>,
    replies: Arc<Mutex<HashMap<RequestId, Waiter>>>,
    pub(super) capabilities: UiCapabilities,
}

/// What the server loop does with the response to one of its requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    CreateProgress,
    OpenDocument,
    RefreshLenses,
    /// A card whose response tells which button the user pressed.
    Card,
}

enum Waiter {
    Task(oneshot::Sender<Response>),
    Server(Purpose),
}

#[derive(Clone, Copy, Default)]
pub(crate) struct UiCapabilities {
    pub show_document: bool,
    pub work_done_progress: bool,
    pub server_status: bool,
    pub lens_refresh: bool,
    /// The client shows a translation itself when sent
    /// [`SHOW_TRANSLATION`], in place of a card.
    pub show_translation: bool,
    /// The client opens its own settings when asked with [`OPEN_SETTINGS`],
    /// in place of Zed's settings file.
    pub open_settings: bool,
    /// The client shows menus and confirmations itself when asked with
    /// [`CHOOSE`], in place of a message with buttons.
    pub choose: bool,
    /// The client draws `$(name)` in Markdown as an icon.
    pub theme_icons: bool,
    /// The client reads Markdown with marked, as it says in the standard
    /// `general.markdown.parser`.
    pub marked: bool,
}

impl UiCapabilities {
    /// Whether the server can open the settings of the client.
    pub fn opens_settings(&self) -> bool {
        self.open_settings || self.show_document
    }

    /// What the client does with Markdown beyond what Zed does.
    pub fn markup(&self) -> Markup {
        Markup {
            theme_icons: self.theme_icons,
            marked: self.marked,
        }
    }
}

impl From<&ClientCapabilities> for UiCapabilities {
    fn from(client: &ClientCapabilities) -> Self {
        Self {
            show_document: client
                .window
                .as_ref()
                .and_then(|window| window.show_document.as_ref())
                .is_some_and(|document| document.support),
            work_done_progress: client
                .window
                .as_ref()
                .is_some_and(|window| window.work_done_progress == Some(true)),
            server_status: client
                .experimental
                .as_ref()
                .is_some_and(|extra| extra["serverStatusNotification"] == true),
            lens_refresh: client
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.code_lens.as_ref())
                .is_some_and(|lens| lens.refresh_support == Some(true)),
            show_translation: named(client, "showTranslation"),
            open_settings: named(client, "openSettings"),
            choose: named(client, "choose"),
            theme_icons: named(client, "themeIcons"),
            marked: client
                .general
                .as_ref()
                .and_then(|general| general.markdown.as_ref())
                .is_some_and(|markdown| markdown.parser == "marked"),
        }
    }
}

/// Whether the client names `item` with `true` under
/// `capabilities.experimental.translationLsp`. Any other value, or none,
/// leaves the item out.
fn named(client: &ClientCapabilities, item: &str) -> bool {
    client
        .experimental
        .as_ref()
        .is_some_and(|extra| extra["translationLsp"][item] == true)
}

/// How the response to a choice names what was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Answers {
    /// The title of a button of `window/showMessageRequest`, as sent.
    Titles(Vec<String>),
    /// The ID of one of this many choices of [`CHOOSE`]: the index of the
    /// choice, written in decimal.
    Ids(usize),
}

impl Answers {
    /// The index of the choice that `result` names. Only its `title` or `id`
    /// is read, so other fields change nothing. A title or ID that names none,
    /// a result without one, and `null` name none.
    pub fn chosen(&self, result: &Value) -> Option<usize> {
        match self {
            Self::Titles(titles) => {
                let title = result.get("title")?.as_str()?;
                titles.iter().position(|choice| choice == title)
            }
            Self::Ids(count) => {
                let id = result.get("id")?.as_str()?;
                (0..*count).find(|index| index.to_string() == id)
            }
        }
    }
}

/// `message` on one line, as a client that names `choose` shows it: each
/// line without the spaces around it, joined with a space.
fn join_lines(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

struct PendingUi {
    ui: Ui,
    id: RequestId,
}

impl Ui {
    pub fn new(sender: Sender<Message>, capabilities: UiCapabilities) -> Self {
        Self {
            sender,
            replies: Arc::default(),
            capabilities,
        }
    }

    /// Hands a response to the task that awaits it, or returns it with its
    /// purpose for the server loop. A response to no live request is dropped.
    pub fn response(&self, response: Response) -> Option<(Purpose, Response)> {
        let waiter = self.replies.lock().unwrap().remove(&response.id)?;
        match waiter {
            Waiter::Task(sender) => {
                let _ = sender.send(response);
                None
            }
            Waiter::Server(purpose) => Some((purpose, response)),
        }
    }

    /// Sends a request whose response [`Ui::response`] returns with `purpose`.
    pub(super) fn send_request(
        &self,
        id: RequestId,
        method: &str,
        params: Value,
        purpose: Purpose,
    ) -> Result<(), Error> {
        self.replies
            .lock()
            .unwrap()
            .insert(id.clone(), Waiter::Server(purpose));
        self.send(id, method, params)
    }

    /// Withdraws a request that is still waiting for its response.
    pub(super) fn cancel_request(&self, id: &RequestId) {
        if self.replies.lock().unwrap().remove(id).is_some() {
            let _ = self
                .sender
                .send(Notification::new("$/cancelRequest".into(), json!({"id": id})).into());
        }
    }

    fn send(&self, id: RequestId, method: &str, params: Value) -> Result<(), Error> {
        self.sender
            .send(
                Request {
                    id,
                    method: method.into(),
                    params,
                }
                .into(),
            )
            .map_err(|_| Error::failed("The editor disconnected"))
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, Error> {
        let id = RequestId::from(format!("translation-ui-{}", Uuid::new_v4()));
        let (sender, reply) = oneshot::channel();
        self.replies
            .lock()
            .unwrap()
            .insert(id.clone(), Waiter::Task(sender));
        let _pending = PendingUi {
            ui: self.clone(),
            id: id.clone(),
        };
        self.send(id, method, params)?;
        let response = reply
            .await
            .map_err(|_| Error::failed("The editor disconnected"))?;
        response
            .response_result
            .map_err(|_| Error::failed("The editor declined the operation"))
    }

    /// Asks the editor to request the CodeLenses of its documents again.
    pub fn refresh_lenses(&self) -> Result<(), Error> {
        if !self.capabilities.lens_refresh {
            return Ok(());
        }
        let id = RequestId::from(format!("translation-ui-{}", Uuid::new_v4()));
        self.send_request(
            id,
            "workspace/codeLens/refresh",
            Value::Null,
            Purpose::RefreshLenses,
        )
    }

    /// Shows `message` in a card with the buttons `actions`, which stays until
    /// the user closes it or presses one. Zed closes a message without actions
    /// after a few seconds by default. A client that names `choose` shows a
    /// menu or a confirmation as `choice` asks. Gives the request whose
    /// response says which was pressed, and how it says so.
    pub fn show_card(
        &self,
        kind: MessageType,
        choice: ChoiceKind,
        message: &str,
        actions: &[&str],
    ) -> Result<(RequestId, Answers), Error> {
        let id = RequestId::from(format!("translation-card-{}", Uuid::new_v4()));
        let (method, params, answers) = self.choice_request(kind, choice, message, actions);
        self.send_request(id.clone(), method, params, Purpose::Card)?;
        Ok((id, answers))
    }

    /// The request that asks the user to pick one of `choices`, and how its
    /// response names the one picked. A client that names `choose` is sent
    /// a menu or a confirmation on one line, with the index of each choice as
    /// its ID; any other, and every message, a message with buttons.
    fn choice_request(
        &self,
        kind: MessageType,
        choice: ChoiceKind,
        message: &str,
        choices: &[impl AsRef<str>],
    ) -> (&'static str, Value, Answers) {
        let titles = choices.iter().map(AsRef::as_ref);
        let kind_name = match choice {
            ChoiceKind::Menu => Some("menu"),
            ChoiceKind::Confirm => Some("confirm"),
            ChoiceKind::Message => None,
        };
        if let Some(kind_name) = kind_name.filter(|_| self.capabilities.choose) {
            let choices: Vec<_> = titles
                .enumerate()
                .map(|(index, title)| json!({"id": index.to_string(), "title": title}))
                .collect();
            let count = choices.len();
            let params =
                json!({"kind": kind_name, "message": join_lines(message), "choices": choices});
            return (CHOOSE, params, Answers::Ids(count));
        }

        let titles: Vec<String> = titles.map(str::to_owned).collect();
        let actions: Vec<_> = titles.iter().map(|title| json!({"title":title})).collect();
        let params = json!({"type":kind,"message":message,"actions":actions});
        ("window/showMessageRequest", params, Answers::Titles(titles))
    }

    pub fn inform(&self, message: &str) -> Result<(), Error> {
        self.show_message(MessageType::INFO, message)
    }

    pub fn warn(&self, message: &str) -> Result<(), Error> {
        self.show_message(MessageType::WARNING, message)
    }

    pub fn error(&self, message: &str) -> Result<(), Error> {
        self.show_message(MessageType::ERROR, message)
    }

    /// Whether the editor reports that it showed `uri`.
    async fn show_document(&self, uri: &str, external: bool) -> Result<bool, Error> {
        let result = self
            .request(
                "window/showDocument",
                json!({"uri":uri,"external":external,"takeFocus":true}),
            )
            .await?;
        Ok(result["success"] == true)
    }

    fn show_message(&self, kind: MessageType, message: &str) -> Result<(), Error> {
        self.notify("window/showMessage", json!({"type":kind,"message":message}))
    }

    /// Hands a translation to a client that names `showTranslation`.
    pub fn show_translation(&self, params: Value) -> Result<(), Error> {
        self.notify(SHOW_TRANSLATION, params)
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        self.sender
            .send(Notification::new(method.into(), params).into())
            .map_err(|_| Error::failed("The editor disconnected"))
    }
}

/// Where Zed keeps the user's settings, as Zed itself locates them.
pub(crate) fn settings_file() -> Option<PathBuf> {
    #[cfg(windows)]
    let directory = dirs::config_dir()?.join("Zed");
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let directory = env::var_os("FLATPAK_XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)?
        .join("zed");
    #[cfg(not(any(windows, target_os = "linux", target_os = "freebsd")))]
    let directory = dirs::home_dir()?.join(".config").join("zed");
    Some(directory.join("settings.json"))
}

impl Drop for PendingUi {
    fn drop(&mut self) {
        self.ui.cancel_request(&self.id);
    }
}

impl Interaction for Ui {
    async fn open(&self, uri: &str) -> Result<(), Error> {
        if !self.capabilities.show_document {
            return Err(Error::failed(
                "The editor must support window/showDocument with external URLs",
            ));
        }
        if !self.show_document(uri, true).await? {
            return Err(Error::failed(
                "The editor could not open the requested page",
            ));
        }
        Ok(())
    }

    async fn open_settings(&self) -> Result<(), Error> {
        if self.capabilities.open_settings {
            return self
                .request(OPEN_SETTINGS, Value::Null)
                .await
                .map(drop)
                .map_err(|_| Error::failed(SETTINGS_NOT_OPENED));
        }
        let unopened = || Error::failed(NO_SETTINGS_FILE);
        if !self.capabilities.show_document {
            return Err(unopened());
        }
        let uri = settings_file()
            .and_then(|path| Url::from_file_path(path).ok())
            .ok_or_else(unopened)?;
        if !self
            .show_document(uri.as_str(), false)
            .await
            .map_err(|_| unopened())?
        {
            return Err(unopened());
        }
        Ok(())
    }

    async fn choose(
        &self,
        kind: ChoiceKind,
        message: &str,
        choices: &[String],
    ) -> Result<Option<usize>, Error> {
        let (method, params, answers) =
            self.choice_request(MessageType::INFO, kind, message, choices);
        let value = self.request(method, params).await?;
        Ok(answers.chosen(&value))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::from_value;

    use super::*;

    fn capabilities(experimental: Value) -> UiCapabilities {
        let client: ClientCapabilities = from_value(json!({"experimental": experimental})).unwrap();
        UiCapabilities::from(&client)
    }

    #[test]
    fn only_items_named_true_are_taken_and_anything_else_is_left_out() {
        let all = capabilities(json!({"translationLsp": {
            "showTranslation": true, "openSettings": true, "choose": true, "themeIcons": true,
            "unknown": true
        }}));
        assert!(all.show_translation && all.open_settings && all.choose && all.theme_icons);
        assert!(all.opens_settings() && !all.show_document);
        let one = capabilities(json!({"translationLsp": {"themeIcons": true}}));
        assert!(one.theme_icons && !one.show_translation && !one.open_settings);
        for experimental in [
            Value::Null,
            json!({}),
            json!({"translationLsp": true}),
            json!({"translationLsp": []}),
            json!({"translationLsp": "showTranslation"}),
            json!({"translationLsp": {
                "showTranslation": "true", "openSettings": 1, "choose": "true", "themeIcons": null
            }}),
            json!({"translationLsp": {"showTranslation": false, "openSettings": {}}}),
            json!({"showTranslation": true, "openSettings": true, "themeIcons": true}),
        ] {
            let none = capabilities(experimental.clone());
            assert!(
                !none.show_translation && !none.open_settings && !none.choose && !none.theme_icons,
                "{experimental}"
            );
            assert!(!none.opens_settings());
        }
    }

    #[test]
    fn an_answer_names_a_choice_only_by_a_title_or_id_that_was_sent() {
        let titles = Answers::Titles(vec!["Same".into(), "Other".into(), "Same".into()]);
        assert_eq!(titles.chosen(&json!({"title": "Other"})), Some(1));
        // A title names the first choice with it, as before.
        assert_eq!(titles.chosen(&json!({"title": "Same"})), Some(0));
        let ids = Answers::Ids(3);
        assert_eq!(ids.chosen(&json!({"id": "0"})), Some(0));
        assert_eq!(ids.chosen(&json!({"id": "2"})), Some(2));
        // Only the ID is read: other fields of the answer change nothing.
        assert_eq!(ids.chosen(&json!({"id": "1", "title": "Other"})), Some(1));
        for answer in [
            Value::Null,
            json!({}),
            json!("0"),
            json!({"id": 1}),
            json!({"id": "3"}),
            json!({"id": "+1"}),
            json!({"id": "01"}),
            json!({"id": " 1"}),
            json!({"title": "Same"}),
        ] {
            assert_eq!(ids.chosen(&answer), None, "{answer}");
        }
        assert_eq!(titles.chosen(&json!({"title": "Missing"})), None);
        assert_eq!(titles.chosen(&json!({"id": "0"})), None);
    }

    #[test]
    fn a_choice_is_sent_as_the_client_shows_it() {
        let (sender, _receiver) = crossbeam_channel::unbounded();
        let named = Ui::new(
            sender.clone(),
            UiCapabilities {
                choose: true,
                ..Default::default()
            },
        );
        let unnamed = Ui::new(sender, UiCapabilities::default());
        let message = "First line  \n(second line)";
        let choices = ["One", "Two"];

        let (method, params, answers) =
            named.choice_request(MessageType::INFO, ChoiceKind::Menu, message, &choices);
        assert_eq!(method, CHOOSE);
        assert_eq!(
            params,
            json!({
                "kind": "menu",
                "message": "First line (second line)",
                "choices": [{"id": "0", "title": "One"}, {"id": "1", "title": "Two"}]
            })
        );
        assert_eq!(answers, Answers::Ids(2));
        let (_, params, _) =
            named.choice_request(MessageType::INFO, ChoiceKind::Confirm, message, &choices);
        assert_eq!(params["kind"], "confirm");

        // A message, and every choice of a client that does not name
        // `choose`, is a message with buttons, as before.
        for (ui, kind) in [
            (&named, ChoiceKind::Message),
            (&unnamed, ChoiceKind::Menu),
            (&unnamed, ChoiceKind::Confirm),
        ] {
            let (method, params, answers) =
                ui.choice_request(MessageType::WARNING, kind, message, &choices);
            assert_eq!(method, "window/showMessageRequest");
            assert_eq!(
                params,
                json!({
                    "type": 2,
                    "message": message,
                    "actions": [{"title": "One"}, {"title": "Two"}]
                })
            );
            assert_eq!(answers, Answers::Titles(vec!["One".into(), "Two".into()]));
        }
    }
}
