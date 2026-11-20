//! The commands that Code Actions carry and `workspace/executeCommand` runs.

use std::collections::HashSet;

use super::{
    menu::{DELETE, RETRY, card_message},
    parts::sent_parts,
    *,
};
use crate::store::{Key, Sent};

pub(super) enum ServerCommand {
    TranslateSelection(TranslationTarget),
    Retry(TranslationTarget),
    /// The translation of a range from its CodeLens, which shows it when it
    /// is ready.
    LensTranslate(TranslationTarget),
    /// A retry from the CodeLens of a range, which shows the translation when
    /// it is ready.
    LensRetry(TranslationTarget),
    /// Shows the translation or failure of a range in a card.
    Show(TranslationTarget),
    OpenTab(TranslationTarget),
    /// A tab translation from the CodeLens of the menu, which translates the
    /// whole document as it is now.
    LensOpenTab(TranslationTarget),
    Cancel(u64),
    CancelAll,
    DeleteTranslation(TranslationTarget),
    DeleteDocumentTranslations(Uri),
    DeleteAllTranslations,
    /// The account menu, or an account command of the provider.
    Account(String),
    /// The server menu.
    Menu(Option<Uri>),
    /// Turns the automatic features back to the settings (`true`) or off.
    Automatic(bool),
    /// Opens Zed's settings file.
    OpenSettings,
}

impl ServerCommand {
    /// Decodes a command, checking its arguments.
    pub fn parse<P: TranslationProvider>(params: ExecuteCommandParams) -> RequestResult<Self> {
        let ExecuteCommandParams {
            command, arguments, ..
        } = params;
        let one = |arguments: Vec<Value>, message: &str| {
            <[Value; 1]>::try_from(arguments)
                .map(|[argument]| argument)
                .map_err(|_| failure(ErrorCode::InvalidParams, message))
        };
        let target = |arguments| parse(one(arguments, "Expected one selection argument")?);
        let none = |arguments: &[Value], message: &str| {
            if arguments.is_empty() {
                Ok(())
            } else {
                Err(failure(ErrorCode::InvalidParams, message))
            }
        };
        match command.as_str() {
            COMMAND => Ok(Self::TranslateSelection(target(arguments)?)),
            RETRY_COMMAND => Ok(Self::Retry(target(arguments)?)),
            LENS_TRANSLATE_COMMAND => Ok(Self::LensTranslate(target(arguments)?)),
            LENS_RETRY_COMMAND => Ok(Self::LensRetry(target(arguments)?)),
            SHOW_COMMAND => Ok(Self::Show(target(arguments)?)),
            DOCUMENT_COMMAND => Ok(Self::OpenTab(target(arguments)?)),
            LENS_DOCUMENT_COMMAND => Ok(Self::LensOpenTab(target(arguments)?)),
            CANCEL_COMMAND => Ok(Self::Cancel(parse(one(
                arguments,
                "Expected one translation ticket",
            )?)?)),
            CANCEL_ALL_COMMAND => {
                none(&arguments, "Cancel all takes no arguments")?;
                Ok(Self::CancelAll)
            }
            DELETE_COMMAND => Ok(Self::DeleteTranslation(target(arguments)?)),
            DELETE_DOCUMENT_COMMAND => Ok(Self::DeleteDocumentTranslations(parse(one(
                arguments,
                "Expected one document URI",
            )?)?)),
            DELETE_ALL_COMMAND => {
                none(&arguments, "Delete all takes no arguments")?;
                Ok(Self::DeleteAllTranslations)
            }
            MENU_COMMAND => Ok(Self::Menu(if arguments.is_empty() {
                None
            } else {
                Some(parse(one(arguments, "Expected at most one document URI")?)?)
            })),
            AUTOMATIC_OFF_COMMAND | AUTOMATIC_ON_COMMAND | OPEN_SETTINGS_COMMAND => {
                none(&arguments, "Menu commands take no arguments")?;
                Ok(match command.as_str() {
                    AUTOMATIC_OFF_COMMAND => Self::Automatic(false),
                    AUTOMATIC_ON_COMMAND => Self::Automatic(true),
                    _ => Self::OpenSettings,
                })
            }
            _ if P::capabilities().account_menu.is_some()
                && (command == SETTINGS_COMMAND
                    || P::capabilities()
                        .commands
                        .iter()
                        .any(|entry| entry.command == command)) =>
            {
                none(&arguments, "Account commands take no arguments")?;
                Ok(Self::Account(command))
            }
            _ => Err(failure(
                ErrorCode::InvalidParams,
                "Unknown translation command",
            )),
        }
    }

    /// The Code Action that carries this command.
    pub fn action(&self, title: String) -> RequestResult<CodeAction> {
        Ok(CodeAction {
            title: title.clone(),
            kind: Some(CodeActionKind::new(ACTION_KIND)),
            command: Some(self.command(title)?),
            ..Default::default()
        })
    }

    pub fn command(&self, title: String) -> RequestResult<Command> {
        let (command, arguments) = match self {
            Self::TranslateSelection(target) => (COMMAND, Some(vec![to_value(target)?])),
            Self::Retry(target) => (RETRY_COMMAND, Some(vec![to_value(target)?])),
            Self::LensTranslate(target) => (LENS_TRANSLATE_COMMAND, Some(vec![to_value(target)?])),
            Self::LensRetry(target) => (LENS_RETRY_COMMAND, Some(vec![to_value(target)?])),
            Self::Show(target) => (SHOW_COMMAND, Some(vec![to_value(target)?])),
            Self::OpenTab(target) => (DOCUMENT_COMMAND, Some(vec![to_value(target)?])),
            Self::LensOpenTab(target) => (LENS_DOCUMENT_COMMAND, Some(vec![to_value(target)?])),
            Self::Cancel(ticket) => (CANCEL_COMMAND, Some(vec![to_value(ticket)?])),
            Self::CancelAll => (CANCEL_ALL_COMMAND, Some(Vec::new())),
            Self::DeleteTranslation(target) => (DELETE_COMMAND, Some(vec![to_value(target)?])),
            Self::DeleteDocumentTranslations(uri) => {
                (DELETE_DOCUMENT_COMMAND, Some(vec![to_value(uri)?]))
            }
            Self::DeleteAllTranslations => (DELETE_ALL_COMMAND, None),
            Self::Account(command) => (command.as_str(), None),
            Self::Menu(uri) => (
                MENU_COMMAND,
                uri.as_ref()
                    .map(|uri| to_value(uri).map(|uri| vec![uri]))
                    .transpose()?,
            ),
            Self::Automatic(true) => (AUTOMATIC_ON_COMMAND, None),
            Self::Automatic(false) => (AUTOMATIC_OFF_COMMAND, None),
            Self::OpenSettings => (OPEN_SETTINGS_COMMAND, None),
        };
        Ok(Command::new(title, command.into(), arguments))
    }
}

/// What a command answers.
pub(super) enum Caller {
    /// A `workspace/executeCommand` request, which the result answers.
    Request(RequestId),
    /// The menu card on which the user chose the command, which nothing
    /// answers. Menus offer no translation commands; the ID only fills the
    /// parameter of `queue`, as for a card's Retry.
    Menu(RequestId),
}

impl Caller {
    /// The request that the result answers, if any.
    fn request(&self) -> Option<RequestId> {
        match self {
            Self::Request(id) => Some(id.clone()),
            Self::Menu(_) => None,
        }
    }

    /// The ID that translation work is queued under.
    fn id(self) -> RequestId {
        match self {
            Self::Request(id) | Self::Menu(id) => id,
        }
    }
}

impl<P: TranslationProvider> Server<P> {
    /// Runs the command of request `id`. `None` means that the request is
    /// answered later.
    pub(super) fn execute_command(
        &mut self,
        connection: &Connection,
        id: RequestId,
        params: ExecuteCommandParams,
    ) -> RequestResult<Option<Value>> {
        self.run_command(connection, Caller::Request(id), params)
    }

    /// Runs a command as things are now, from a request or a menu.
    pub(super) fn run_command(
        &mut self,
        connection: &Connection,
        caller: Caller,
        params: ExecuteCommandParams,
    ) -> RequestResult<Option<Value>> {
        if params.command == LENS_COMMAND {
            return Ok(Some(Value::Null));
        }
        let command = ServerCommand::parse::<P>(params)?;
        if let ServerCommand::TranslateSelection(target)
        | ServerCommand::Retry(target)
        | ServerCommand::LensTranslate(target)
        | ServerCommand::LensRetry(target)
        | ServerCommand::Show(target)
        | ServerCommand::DeleteTranslation(target)
        | ServerCommand::OpenTab(target)
        | ServerCommand::LensOpenTab(target) = &command
            && target.target_language != self.settings.target_language
        {
            return Err(failure(
                ErrorCode::InvalidParams,
                "The target language must match initialization_options",
            ));
        }
        let command = match command {
            ServerCommand::LensTranslate(target) => {
                ServerCommand::LensTranslate(self.current_lens_target(target, true)?)
            }
            ServerCommand::LensRetry(target) => {
                ServerCommand::LensRetry(self.current_lens_target(target, false)?)
            }
            ServerCommand::Show(target) => {
                ServerCommand::Show(self.current_lens_target(target, false)?)
            }
            ServerCommand::LensOpenTab(target) => {
                ServerCommand::LensOpenTab(self.current_tab_target(target))
            }
            command => command,
        };
        match command {
            ServerCommand::CancelAll => {
                self.cancel_translations(connection)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::Cancel(ticket) => {
                self.cancel_range(connection, ticket, ErrorCode::RequestCanceled)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::DeleteTranslation(target) => {
                self.delete_translation(&target).map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::DeleteDocumentTranslations(uri) => {
                self.delete_document_translations(&uri)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::DeleteAllTranslations => {
                self.confirm_delete_all().map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::Account(command) if command == SETTINGS_COMMAND => {
                let choices = self
                    .provider
                    .account_commands()
                    .into_iter()
                    .map(|entry| Command::new(entry.title.into(), entry.command.into(), None))
                    .collect();
                self.open_menu("", choices).map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::Account(command) => self.start_administration(
                connection,
                caller.request(),
                Task::Administration {
                    command,
                    ui: self.ui.clone(),
                },
            ),
            ServerCommand::Menu(uri) => {
                let choices = self.menu_choices(uri.as_ref())?;
                self.open_menu(self.menu_message(), choices)
                    .map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::Automatic(on) => {
                self.set_automatic(connection, on)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::OpenSettings => {
                if !self.ui.capabilities.show_document {
                    return Err(failure(
                        ErrorCode::RequestFailed,
                        "The editor does not support opening files",
                    ));
                }
                self.start_administration(
                    connection,
                    caller.request(),
                    Task::OpenSettings {
                        ui: self.ui.clone(),
                    },
                )
            }
            ServerCommand::OpenTab(target) | ServerCommand::LensOpenTab(target) => {
                if !self.ui.capabilities.show_document {
                    return Err(failure(
                        ErrorCode::RequestFailed,
                        "The editor does not support opening translation documents",
                    ));
                }
                if !self
                    .documents
                    .get(&target.uri)
                    .is_some_and(Document::supports_document_preview)
                {
                    return Err(failure(
                        ErrorCode::InvalidParams,
                        "Tab translations support Markdown and Plain Text",
                    ));
                }
                self.check_feature(Feature::DocumentAction, &target)?;
                self.queue_translation(target, TranslationRequest::Document)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::TranslateSelection(target) => {
                self.check_feature(Feature::SelectionAction, &target)?;
                self.queue(caller.id(), target, PendingReply::Command { notice: true })
            }
            ServerCommand::LensTranslate(target) => {
                self.check_feature(Feature::SelectionAction, &target)?;
                self.queue(caller.id(), target, PendingReply::Command { notice: false })
            }
            ServerCommand::Retry(target) => {
                self.queue(caller.id(), target, PendingReply::Retry { notice: true })
            }
            ServerCommand::LensRetry(target) => {
                self.queue(caller.id(), target, PendingReply::Retry { notice: false })
            }
            ServerCommand::Show(target) => {
                let gone = || {
                    failure(
                        ErrorCode::ContentModified,
                        "The translation is no longer available",
                    )
                };
                if !self.target_is_current(&target) {
                    return Err(gone());
                }
                self.check_cache()?;
                let (kind, message) = match self
                    .cache
                    .message(&target.uri, target.range)
                    .ok_or_else(gone)?
                {
                    Ok(translation) => (MessageType::INFO, translation),
                    Err(reason) => (MessageType::ERROR, reason),
                };
                let card = self
                    .ui
                    .show_card(kind, &card_message(&message), &[RETRY, DELETE])
                    .map_err(provider_failure)?;
                self.cards.insert(card, Card::Translation(target));
                Ok(Some(Value::Null))
            }
        }
    }

    /// The target of a CodeLens command in the current text. Zed keeps a
    /// CodeLens line that shows the same text when it asks for CodeLenses
    /// again, with the command of the version it was made for, so a target of
    /// an older version whose range no edit since touched is moved to where
    /// that range is now. In code, the range must still be a whole comment:
    /// a comment line added right before it can join the two.
    ///
    /// With `translate`, in code, only the start of the range is followed,
    /// and the whole comment that starts there now is the target: the
    /// `Translate` line of a comment stays as it was while the comment is
    /// written or edited. Any other target is left as it is, for the checks
    /// after this to refuse.
    fn current_lens_target(
        &mut self,
        target: TranslationTarget,
        translate: bool,
    ) -> RequestResult<TranslationTarget> {
        let Some(document) = self.documents.get_mut(&target.uri) else {
            return Ok(target);
        };
        if document.version == target.version {
            return Ok(target);
        }
        let code = !document.language.is_some_and(Language::is_prose);
        let followed = if translate && code {
            let start = target.range.start;
            Range::new(start, start)
        } else {
            target.range
        };
        let Some(range) = document.current_range(target.session, target.version, followed) else {
            return Ok(target);
        };
        let range = if code {
            let limit = Duration::from_millis(self.settings.comment_parse_timeout_ms);
            let comment = document.comment_starting_at(&target.uri, range.start, limit);
            report_comment_failure(&self.ui, &target.uri, document)?;
            match comment {
                Some(comment) if translate || comment == range => comment,
                _ => return Ok(target),
            }
        } else {
            range
        };
        Ok(TranslationTarget {
            version: document.version,
            range,
            ..target
        })
    }

    /// The target of `Translate in tab` in the current text: the whole
    /// document as it is now, whatever version the CodeLens line was made
    /// for, if the document is in the same open lifetime and synchronized.
    /// Any other target is left as it is, for the checks after this to
    /// refuse.
    fn current_tab_target(&self, target: TranslationTarget) -> TranslationTarget {
        let Some(document) = self
            .documents
            .get(&target.uri)
            .filter(|document| document.session == target.session)
        else {
            return target;
        };
        match document.full_range() {
            Some(range) => TranslationTarget {
                version: document.version,
                range,
                ..target
            },
            None => target,
        }
    }

    pub(super) fn delete_translation(
        &mut self,
        target: &TranslationTarget,
    ) -> Result<(), ProviderError> {
        if !self.target_is_current(target) {
            return Ok(());
        }

        let keys = self.cache.remove_range(&target.uri, target.range);
        self.store.remove(&keys);
        self.clear_deleted_problem(&target.uri, Some(target.range));
        self.ui.refresh_lenses()
    }

    /// Deletes the results of the document, and the stored translations of
    /// what its text sends now, which may have none in the cache, such as
    /// after a restart.
    fn delete_document_translations(&mut self, uri: &Uri) -> RequestResult<()> {
        let mut keys = self.cache.remove_document(uri);
        keys.extend(self.document_keys(uri)?);
        self.store.remove(&keys);
        self.clear_deleted_problem(uri, None);
        self.ui.refresh_lenses().map_err(provider_failure)
    }

    /// The keys of what the current text of the document sends with the
    /// settings, each once: the whole text, for ranges and tabs, and its
    /// parts if a tab sends it in parts, unless the text is too long to send;
    /// and in code, each comment, translated or found in the target language.
    fn document_keys(&mut self, uri: &Uri) -> RequestResult<Vec<Key>> {
        let Some(document) = self.documents.get_mut(uri) else {
            return Ok(Vec::new());
        };
        let target = self.settings.target_language.as_str();
        let mut keys = Vec::new();
        if let Some(text) = document.text()
            && text.len() <= self.settings.translation_limits.input_kib.document.bytes()
        {
            for output in [Output::Range, Output::Document] {
                keys.push(self.store.key(output, target, Sent::Passage(text)));
            }
            let split = self.settings.document_split;
            if split.enabled && document.supports_document_preview() {
                let markdown = document.language == Some(Language::Markdown);
                for part in sent_parts(text, markdown, split) {
                    keys.push(self.store.key(Output::Document, target, Sent::Part(part)));
                }
            }
        }
        if !document.language.is_some_and(Language::is_prose) {
            let limit = Duration::from_millis(self.settings.comment_parse_timeout_ms);
            for body in document.comment_bodies(uri, limit) {
                keys.push(self.store.key(Output::Range, target, Sent::Passage(&body)));
                keys.push(
                    self.store
                        .key(Output::Range, target, Sent::SameLanguage(&body)),
                );
            }
            report_comment_failure(&self.ui, uri, document)?;
        }
        let mut seen = HashSet::new();
        keys.retain(|key| seen.insert(*key));
        Ok(keys)
    }

    fn clear_deleted_problem(&mut self, uri: &Uri, range: Option<Range>) {
        if self.problem.as_ref().is_some_and(|problem| {
            &problem.target.uri == uri && range.is_none_or(|range| problem.target.range == range)
        }) {
            self.clear_problem();
        }
    }

    /// The actions of a feature turned off are not listed, so its command can
    /// only come from a client that did not take it from the list.
    fn check_feature(&self, feature: Feature, target: &TranslationTarget) -> RequestResult<()> {
        if self
            .documents
            .get(&target.uri)
            .is_some_and(|document| !self.settings.features.enables(feature, document.language))
        {
            return Err(failure(
                ErrorCode::InvalidParams,
                &format!("The features setting turns off {feature} for this language"),
            ));
        }
        Ok(())
    }
}
