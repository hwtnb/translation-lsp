//! The commands that Code Actions carry and `workspace/executeCommand` runs.

use super::{
    menu::{DELETE, RETRY, card_message},
    *,
};

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

impl<P: TranslationProvider> Server<P> {
    pub(super) fn execute_command(
        &mut self,
        connection: &Connection,
        id: RequestId,
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
        | ServerCommand::OpenTab(target) = &command
            && target.target_language != self.settings.target_language
        {
            return Err(failure(
                ErrorCode::InvalidParams,
                "The target language must match initialization_options",
            ));
        }
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
                self.delete_document_translations(&uri)
                    .map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::DeleteAllTranslations => {
                self.confirm_delete_all().map_err(provider_failure)?;
                Ok(Some(Value::Null))
            }
            ServerCommand::Account(command) => {
                let ui = self.ui.clone();
                let task = if command == SETTINGS_COMMAND {
                    Task::AccountMenu { ui }
                } else {
                    Task::Administration { command, ui }
                };
                self.start_administration(connection, id, task)
            }
            ServerCommand::Menu(uri) => self.start_administration(
                connection,
                id,
                Task::Menu {
                    ui: self.ui.clone(),
                    message: self.menu_message().into(),
                    choices: self.menu_choices(uri.as_ref())?,
                },
            ),
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
                    id,
                    Task::OpenSettings {
                        ui: self.ui.clone(),
                    },
                )
            }
            ServerCommand::OpenTab(target) => {
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
                self.queue(id, target, PendingReply::Command { notice: true })
            }
            ServerCommand::LensTranslate(target) => {
                self.check_feature(Feature::SelectionAction, &target)?;
                self.queue(id, target, PendingReply::Command { notice: false })
            }
            ServerCommand::Retry(target) => {
                self.queue(id, target, PendingReply::Retry { notice: true })
            }
            ServerCommand::LensRetry(target) => {
                self.queue(id, target, PendingReply::Retry { notice: false })
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

    pub(super) fn delete_translation(
        &mut self,
        target: &TranslationTarget,
    ) -> Result<(), ProviderError> {
        if !self.target_is_current(target) {
            return Ok(());
        }

        self.cache.remove_range(&target.uri, target.range);
        self.clear_deleted_problem(&target.uri, Some(target.range));
        self.ui.refresh_lenses()
    }

    fn delete_document_translations(&mut self, uri: &Uri) -> Result<(), ProviderError> {
        self.cache.remove_document(uri);
        self.clear_deleted_problem(uri, None);
        self.ui.refresh_lenses()
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
