//! The server menu that the Settings CodeLens opens.

use super::*;

/// The menu message while the automatic features follow the settings.
const AUTOMATIC_ON: &str =
    "Automatic translation: On  \n(comment Hover and automatic selection translation)";
/// The menu message after the user turned the automatic features off.
const AUTOMATIC_OFF: &str = "Automatic translation: Off until the language server restarts";
/// The button of a translation card that translates its range again.
pub(super) const RETRY: &str = "Retry translating this text";
/// The button of a translation card that removes its cached result.
pub(super) const DELETE: &str = "Delete translation";
pub(super) const DELETE_DOCUMENT: &str = "Delete translations of this document";
pub(super) const DELETE_ALL: &str = "Delete all cached translations";
const DELETE_ALL_QUESTION: &str = "Delete all cached translations? Comments and selections are translated again when you ask for them.";
const CONFIRM_DELETE: &str = "Delete";

pub(super) enum Card {
    Translation(TranslationTarget),
    ConfirmDeleteAll,
}

/// The Markdown of a translation card around `text`. Zed draws a rule in a
/// card with no color, so the rules only set the text apart from the card's
/// header and button with space.
pub(super) fn card_message(text: &str) -> String {
    format!("---\n\n{text}\n\n---")
}

impl<P: TranslationProvider> Server<P> {
    pub(super) fn menu_message(&self) -> &'static str {
        if self.settings.features.automatic_paused() {
            AUTOMATIC_OFF
        } else {
            AUTOMATIC_ON
        }
    }

    /// The titles of the server menu, each with the command it runs.
    pub(super) fn menu_choices(&self, uri: Option<&Uri>) -> RequestResult<Vec<Command>> {
        let automatic = if self.settings.features.automatic_paused() {
            (
                "Turn on automatic translation",
                ServerCommand::Automatic(true),
            )
        } else {
            (
                "Turn off automatic translation",
                ServerCommand::Automatic(false),
            )
        };
        [automatic]
            .into_iter()
            .chain(
                P::capabilities()
                    .account_menu
                    .map(|title| (title, ServerCommand::Account(SETTINGS_COMMAND.into()))),
            )
            .chain(
                self.ui
                    .capabilities
                    .show_document
                    .then_some(("Open Zed settings file", ServerCommand::OpenSettings)),
            )
            .chain(uri.filter(|uri| self.can_delete_document(uri)).map(|uri| {
                (
                    DELETE_DOCUMENT,
                    ServerCommand::DeleteDocumentTranslations(uri.clone()),
                )
            }))
            .chain(
                (!self.cache.is_empty() || self.has_stored())
                    .then_some((DELETE_ALL, ServerCommand::DeleteAllTranslations)),
            )
            .map(|(title, command)| command.command(title.into()))
            .collect()
    }

    /// The menu as CodeLenses on a line above the first one, starting with
    /// the tab translation of the whole document if `tab` gives its target.
    pub(super) fn menu_lenses(&self, tab: Option<Vec<Value>>) -> Vec<CodeLens> {
        let (state, command) = if self.settings.features.automatic_paused() {
            ("Off", AUTOMATIC_ON_COMMAND)
        } else {
            ("On", AUTOMATIC_OFF_COMMAND)
        };
        let tab = tab.map(|target| {
            lens(
                Range::default(),
                "Translate in tab".into(),
                LENS_DOCUMENT_COMMAND,
                Some(target),
            )
        });
        let menu = [(format!("Auto-translate: {state}"), command)]
            .into_iter()
            .chain(
                P::capabilities()
                    .account_menu
                    .map(|title| (title.to_owned(), SETTINGS_COMMAND)),
            )
            .chain(
                self.ui
                    .capabilities
                    .show_document
                    .then(|| ("Zed settings".to_owned(), OPEN_SETTINGS_COMMAND)),
            )
            .map(|(title, command)| lens(Range::default(), title, command, None));
        let mut lenses: Vec<_> = tab.into_iter().chain(menu).collect();
        if let Some(first) = lenses.first_mut().and_then(|lens| lens.command.as_mut()) {
            first.title = format!("Translation LSP: {}", first.title);
        }
        lenses
    }

    pub(super) fn confirm_delete_all(&mut self) -> Result<(), ProviderError> {
        let id = self
            .ui
            .show_card(MessageType::INFO, DELETE_ALL_QUESTION, &[CONFIRM_DELETE])?;
        self.cards.insert(id, Card::ConfirmDeleteAll);
        Ok(())
    }

    /// Runs the action selected on a card that is still open.
    pub(super) fn card_response(&mut self, response: Response) -> ServerResult<()> {
        let Some(card) = self.cards.remove(&response.id) else {
            return Ok(());
        };

        let title = response
            .response_result
            .as_ref()
            .ok()
            .and_then(|value| value["title"].as_str());
        match (card, title) {
            (Card::ConfirmDeleteAll, Some(CONFIRM_DELETE)) => {
                self.clear_previews();
                self.store.clear();
                self.ui.refresh_lenses()?;
            }
            (Card::Translation(target), Some(DELETE)) => self.delete_translation(&target)?,
            (Card::Translation(target), Some(RETRY)) => {
                // A retry answers no request, so the card's request stands in for one.
                if let Err(error) =
                    self.queue(response.id, target, PendingReply::Retry { notice: false })
                {
                    self.ui.error(&error.message)?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    /// Turns comment Hover and automatic selection translation back to the
    /// settings, or off. Turning them off cancels the translations that only
    /// they wait for.
    pub(super) fn set_automatic(&mut self, connection: &Connection, on: bool) -> RequestResult<()> {
        self.settings.features.pause_automatic(!on);
        self.ui.refresh_lenses().map_err(provider_failure)?;
        if on {
            return Ok(());
        }
        self.suppress_selection();
        let tickets: Vec<u64> = self
            .scheduler
            .translations()
            .iter()
            .filter(|(_, work)| work.owners.automatic_only())
            .map(|(ticket, _)| *ticket)
            .collect();
        for ticket in tickets {
            self.cancel_range(connection, ticket, ErrorCode::RequestCanceled)?;
        }
        Ok(())
    }
}
