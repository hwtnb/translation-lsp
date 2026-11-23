//! PLaMo 3 Translate at a service that `provider.base_url` names and that
//! takes the Chat Completions requests of the OpenAI API, such as Ollama. The
//! model reads no instructions, so the system message holds only its settings
//! and the text is sent as written. It names no language either: an automatic
//! translation that comes back as it was sent shows nothing.

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use serde_json::json;

use super::{
    AuthUiState, Capabilities, Intent, Interaction, Kind, Output, ProviderCommand, Translated,
    TranslationProvider,
    api_key::KeyId,
    chat_completions::{COMMANDS, ChatCompletions, MENU, Names},
    error::{Error, Service, credentials::CredentialError},
};
#[cfg(test)]
use crate::config::limits::TranslationLimits;
use crate::{config::limits::SizeLimit, store::Identity};

const NAMES: Names = Names {
    service: Service::PlamoTranslate,
    short: "PLaMo Translate",
};

/// The tokens that begin the model's next message, as the model card stops
/// at them.
const STOP: [&str; 2] = ["<|plamo:tag|>", "<|plamo:bos|>"];

#[derive(Clone)]
pub(crate) struct PlamoProvider {
    chat: ChatCompletions,
    model: String,
    /// A language code, or `auto`.
    source_language: String,
}

impl PlamoProvider {
    /// `base_url` has passed the settings checks. `key_directory` is the
    /// credential directory, where the key file is.
    pub fn new(
        base_url: &str,
        model: String,
        source_language: String,
        output_limit: SizeLimit,
        key_directory: Result<PathBuf, CredentialError>,
    ) -> Result<Self, Error> {
        Ok(Self {
            chat: ChatCompletions::new(base_url, NAMES, output_limit, key_directory)?,
            model,
            source_language,
        })
    }

    #[cfg(test)]
    pub fn synthetic(base_url: &str, source_language: &str, directory: &Path) -> Self {
        let provider = Self::new(
            base_url,
            "synthetic/plamo".into(),
            source_language.into(),
            TranslationLimits::default().output_kib,
            Ok(directory.into()),
        )
        .unwrap();
        Self {
            chat: provider.chat.synthetic(),
            ..provider
        }
    }

    /// The system message: the line the model's settings start with, then
    /// one setting a line.
    fn settings(&self, target_language: &str) -> String {
        let mut settings = format!("[plamo-translate]\ntarget_lang={target_language}");
        if self.source_language != "auto" {
            settings.push_str("\nsource_lang=");
            settings.push_str(&self.source_language);
        }
        settings
    }
}

impl TranslationProvider for PlamoProvider {
    /// The key in use, or none, as with the OpenAI-compatible provider.
    type Context = Option<KeyId>;

    fn identity(&self, _output: Output) -> Identity {
        Identity::new("plamo_translate")
            .with("base_url", self.chat.base_url())
            .with("model", &self.model)
    }

    fn context(&self) -> Result<Option<KeyId>, Error> {
        self.chat.context()
    }

    fn admit(&self, key: &Option<KeyId>, intent: Intent) -> Result<u64, Error> {
        self.chat.admit(key, intent)
    }

    fn check_admission(&self, key: &Option<KeyId>, generation: u64) -> Result<(), Error> {
        self.chat.check_admission(key, generation)
    }

    fn auth_ui_state(&self) -> AuthUiState {
        self.chat.auth_ui_state()
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            server_name: "Translation LSP (PLaMo Translate)",
            automatic_selection: true,
            account_menu: Some(MENU),
            commands: &COMMANDS,
            hover_note: None,
            comment_notice: None,
            range_output_bounded: true,
        }
    }

    fn account_commands(&self) -> Vec<ProviderCommand> {
        self.chat.account_commands()
    }

    async fn translate(
        &self,
        intent: Intent,
        output: Output,
        text: &str,
        target_language: &str,
        expected: Option<KeyId>,
    ) -> Result<Translated<Option<KeyId>>, Error> {
        let mut request = json!({
            "model": self.model,
            "stream": true,
            "temperature": 0,
            "stop": STOP,
            "messages": [
                {"role": "system", "content": self.settings(target_language)},
                {"role": "user", "content": text},
            ],
        });
        if output == Output::Range {
            request["max_tokens"] = json!(self.chat.range_max_tokens(text));
        }
        let reply = self.chat.send(intent, &request, &[text], expected).await?;
        // The model names no language, so a reply that repeats the text is
        // the only sign that the text is already in the target language.
        if intent == Intent::Automatic
            && self.source_language == "auto"
            && reply.trim() == text.trim()
        {
            return Ok(Translated {
                text: String::new(),
                context: expected,
                kind: Kind::SameLanguage,
            });
        }
        if reply.trim().is_empty() {
            return Err(Error::failed(format!(
                "{} completed without translation text",
                NAMES.service
            )));
        }
        Ok(Translated {
            text: reply,
            context: expected,
            kind: Kind::Translation,
        })
    }

    async fn administer(&self, command: &str, ui: &impl Interaction) -> Result<String, Error> {
        self.chat.administer(command, ui).await
    }
}

#[cfg(test)]
mod tests;
