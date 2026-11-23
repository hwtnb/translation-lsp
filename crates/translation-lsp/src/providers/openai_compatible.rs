//! A service that `provider.base_url` names and that takes the Chat
//! Completions requests of the OpenAI API, such as OpenRouter, Gemini, Ollama,
//! or LM Studio. A key is sent only when one is saved for its origin.

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use serde_json::json;

use super::{
    AuthUiState, Capabilities, Intent, Interaction, Output, OutputOptions, ProviderCommand,
    Translated, TranslationProvider,
    api_key::KeyId,
    chat_completions::{COMMANDS, ChatCompletions, MENU, Names},
    dictionary,
    error::{Error, Service, credentials::CredentialError},
    prompt::{self, Form},
};
#[cfg(test)]
use crate::config::limits::TranslationLimits;
use crate::{
    config::{CompatibleModel, limits::SizeLimit},
    store::Identity,
};

const SERVICE: Service = Service::Compatible;
pub(crate) const NAMES: Names = Names {
    service: SERVICE,
    short: "OpenAI-compatible",
};

#[derive(Clone)]
pub(crate) struct CompatibleProvider {
    chat: ChatCompletions,
    options: OutputOptions<CompatibleModel>,
    source_language: String,
}

impl CompatibleProvider {
    /// `base_url` has passed the settings checks. `key_directory` is the
    /// credential directory, where the key file is.
    pub fn new(
        base_url: &str,
        options: OutputOptions<CompatibleModel>,
        source_language: String,
        output_limit: SizeLimit,
        key_directory: Result<PathBuf, CredentialError>,
    ) -> Result<Self, Error> {
        Ok(Self {
            chat: ChatCompletions::new(base_url, NAMES, output_limit, key_directory)?,
            options,
            source_language,
        })
    }

    #[cfg(test)]
    pub fn synthetic(base_url: &str, directory: &Path) -> Self {
        let provider = Self::new(
            base_url,
            OutputOptions::new(
                CompatibleModel {
                    model: "synthetic/model".into(),
                    effort: None,
                },
                None,
            ),
            "auto".into(),
            TranslationLimits::default().output_kib,
            Ok(directory.into()),
        )
        .unwrap();
        Self {
            chat: provider.chat.synthetic(),
            ..provider
        }
    }

    #[cfg(test)]
    pub fn with_options(mut self, options: OutputOptions<CompatibleModel>) -> Self {
        self.options = options;
        self
    }

    /// Sends `message` after the `instructions` and returns the reply. `texts`
    /// are the document's texts in the message, the first sizing the reply;
    /// errors never show them.
    async fn send(
        &self,
        intent: Intent,
        output: Output,
        instructions: String,
        message: String,
        texts: &[&str],
        expected: Option<KeyId>,
    ) -> Result<String, Error> {
        let options = self.options.get(output);
        let mut request = json!({
            "model": options.model,
            "stream": true,
            "messages": [
                {"role": "system", "content": instructions},
                {"role": "user", "content": message},
            ],
        });
        if output == Output::Range {
            request["max_tokens"] = json!(self.chat.range_max_tokens(texts[0]));
        }
        if let Some(effort) = &options.effort {
            request["reasoning_effort"] = json!(effort);
        }
        self.chat.send(intent, &request, texts, expected).await
    }
}

impl TranslationProvider for CompatibleProvider {
    /// The key in use, or none: what was translated or refused with one key
    /// is not used from the cache by position with another, or with none.
    type Context = Option<KeyId>;

    fn identity(&self, output: Output) -> Identity {
        let options = self.options.get(output);
        Identity::new("openai_compatible")
            .with("base_url", self.chat.base_url())
            .with("model", &options.model)
            .with_some("effort", options.effort.as_ref())
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
            server_name: "Translation LSP (OpenAI-compatible)",
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
        let instructions = prompt::instructions(
            target_language,
            &self.source_language,
            intent,
            Form::Translates,
        );
        let reply = self
            .send(
                intent,
                output,
                instructions,
                prompt::message(text),
                &[text],
                expected,
            )
            .await?;
        prompt::translated(
            reply,
            intent,
            &self.source_language,
            target_language,
            SERVICE,
            expected,
        )
    }

    async fn translate_part(
        &self,
        text: &str,
        target_language: &str,
        expected: Option<KeyId>,
    ) -> Result<Translated<Option<KeyId>>, Error> {
        let instructions = prompt::part_instructions(target_language, &self.source_language);
        let reply = self
            .send(
                Intent::Explicit,
                Output::Document,
                instructions,
                prompt::message(text),
                &[text],
                expected,
            )
            .await?;
        prompt::translated(
            reply,
            Intent::Explicit,
            &self.source_language,
            target_language,
            SERVICE,
            expected,
        )
    }

    async fn look_up(
        &self,
        intent: Intent,
        output: Output,
        term: &str,
        line: &str,
        target_language: &str,
        expected: Option<KeyId>,
    ) -> Result<Translated<Option<KeyId>>, Error> {
        let instructions = dictionary::instructions(target_language, &self.source_language);
        let message = dictionary::message(term, line);
        let reply = self
            .send(
                intent,
                output,
                instructions,
                message,
                &[line, term],
                expected,
            )
            .await?;
        dictionary::entry(term, &reply, SERVICE, expected)
    }

    async fn administer(&self, command: &str, ui: &impl Interaction) -> Result<String, Error> {
        self.chat.administer(command, ui).await
    }
}

#[cfg(test)]
mod tests;
