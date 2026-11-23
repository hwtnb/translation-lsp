//! A service that `provider.base_url` names and that takes the Chat
//! Completions requests of the OpenAI API, which the OpenAI-compatible and
//! PLaMo Translate providers share with the key saved for its origin. A key
//! is sent only when one is saved.

mod response;

use std::path::PathBuf;

use reqwest::{Url, header::AUTHORIZATION};
use serde_json::Value;

#[cfg(test)]
use super::api_key::test_support::Terminal as SyntheticTerminal;

use super::{
    AccountChange, AuthUiState, Intent, Interaction, ProviderCommand,
    admission::{Admission, Attempt},
    api_key::{KeyFile, KeyId, KeySpec, Origin, Terminal},
    error::{Error, Service, credentials::CredentialError},
    range_output_tokens, transport,
};
use crate::config::limits::SizeLimit;

/// How errors name a service of this kind.
#[derive(Clone, Copy)]
pub(crate) struct Names {
    pub service: Service,
    /// The name before "request failed" and "HTTP client".
    pub short: &'static str,
}

pub(crate) const MENU: &str = "OpenAI-compatible API key…";

/// What to do when the service refuses a request (HTTP 401), which went to
/// `origin` with a saved key or without one.
fn rejected(origin: &Origin, keyed: bool) -> String {
    if keyed {
        format!(
            "The service did not accept the key saved for {}. Run \"{MENU}\" from the code actions to replace it",
            origin.as_str()
        )
    } else {
        format!(
            "No key is saved for {}, where provider.base_url sends requests. Run \"{MENU}\" from the code actions to save one",
            origin.as_str()
        )
    }
}

const SAVE: &str = "translation-lsp.save-compatible-key";
const REMOVE: &str = "translation-lsp.remove-compatible-key";
pub(crate) const COMMANDS: [ProviderCommand; 2] = [
    // Saving happens later, in the terminal. The server follows the file.
    ProviderCommand {
        command: SAVE,
        title: "Save OpenAI-compatible API key",
        changes_account: AccountChange::Never,
    },
    ProviderCommand {
        command: REMOVE,
        title: "Remove OpenAI-compatible API key",
        changes_account: AccountChange::IfConfirmed,
    },
];
const REMOVAL: &str = "Remove the API key saved for this service from this computer? The key stays valid until you delete it at the service.";

fn key_commands(saved: bool) -> Vec<ProviderCommand> {
    let [mut save, remove] = COMMANDS;
    if saved {
        save.title = "Replace OpenAI-compatible API key";
        vec![save, remove]
    } else {
        vec![save]
    }
}

#[derive(Clone)]
pub(crate) struct ChatCompletions {
    client: reqwest::Client,
    key: KeyFile,
    terminal: Terminal,
    names: Names,
    output_limit: SizeLimit,
    /// Where requests go: `provider.base_url` with `/chat/completions`.
    endpoint: String,
    admission: Admission<Option<KeyId>>,
}

impl ChatCompletions {
    /// `base_url` has passed the settings checks. `key_directory` is the
    /// credential directory, where the key file is.
    pub fn new(
        base_url: &str,
        names: Names,
        output_limit: SizeLimit,
        key_directory: Result<PathBuf, CredentialError>,
    ) -> Result<Self, Error> {
        let origin = Url::parse(base_url)
            .ok()
            .as_ref()
            .and_then(Origin::of)
            .ok_or_else(|| Error::configuration("provider.base_url cannot be used"))?;
        Ok(Self {
            // The worker owns the total deadline.
            client: transport::client_builder().build().map_err(|_| {
                Error::failed(format!("Could not prepare the {} HTTP client", names.short))
            })?,
            key: KeyFile::new(
                KeySpec::origin("OpenAI-compatible API key", MENU, origin),
                key_directory,
            ),
            terminal: Terminal::System,
            names,
            output_limit,
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
            admission: Admission::default(),
        })
    }

    /// Asks for keys on a synthetic terminal instead of the system's.
    #[cfg(test)]
    pub fn synthetic(self) -> Self {
        Self {
            terminal: Terminal::Synthetic(SyntheticTerminal::default()),
            ..self
        }
    }

    /// `provider.base_url` without the slash it may end with.
    pub fn base_url(&self) -> &str {
        self.endpoint
            .strip_suffix("/chat/completions")
            .unwrap_or(&self.endpoint)
    }

    /// The `max_tokens` of a range request whose reply `source` sizes. A tab
    /// is bounded by `translation_limits.output_kib` alone, and a limit above
    /// a model's own may be refused.
    pub fn range_max_tokens(&self, source: &str) -> usize {
        range_output_tokens(source).min(self.output_limit.bytes())
    }

    /// Sends `request`, without its `Authorization`, and returns the reply.
    /// `texts` are the document's texts in the request; errors never show
    /// them.
    pub async fn send(
        &self,
        intent: Intent,
        request: &Value,
        texts: &[&str],
        expected: Option<KeyId>,
    ) -> Result<String, Error> {
        let key = self.key.optional_in_use(expected)?;
        let secrets: Vec<&str> = key
            .as_ref()
            .map(|key| &*key.text)
            .into_iter()
            .chain(texts.iter().copied())
            .collect();
        let rejected = rejected(
            self.key
                .destination()
                .expect("an OpenAI-compatible key belongs to an origin"),
            key.is_some(),
        );
        let names = self.names;
        let in_use = || self.key.optional_in_use(expected).map(drop);
        // Dropping this future on cancellation or expiry drops the request or
        // response it is awaiting, which closes the connection.
        self.admission
            .send(&expected, intent, in_use, async || {
                let mut sending = self.client.post(&self.endpoint).json(request);
                if let Some(key) = &key {
                    sending = sending.header(AUTHORIZATION, key.bearer());
                }
                let response = match sending.send().await {
                    Ok(response) => response,
                    Err(error) => {
                        return Attempt::without_response(transport::network_error(names.service)(
                            error,
                        ));
                    }
                };
                let retry_after = transport::retry_delay(response.headers());
                let result = if response.status().is_success() {
                    response::text(response, names, &secrets, &rejected, self.output_limit).await
                } else {
                    Err(response::failure_of(response, names, &secrets, &rejected).await)
                };
                Attempt {
                    result,
                    retry_after,
                }
            })
            .await
    }

    /// The key in use, or none.
    pub fn context(&self) -> Result<Option<KeyId>, Error> {
        Ok(self.key.read_optional()?.map(|(_, key)| key))
    }

    pub fn admit(&self, key: &Option<KeyId>, intent: Intent) -> Result<u64, Error> {
        self.admission.admit(key, intent)
    }

    pub fn check_admission(&self, key: &Option<KeyId>, generation: u64) -> Result<(), Error> {
        self.admission.check(key, generation)
    }

    /// A service may take requests without a key, so only a key file that
    /// cannot be read keeps translations from starting.
    pub fn auth_ui_state(&self) -> AuthUiState {
        if self.key.read_optional().is_ok() {
            AuthUiState::Ready
        } else {
            AuthUiState::SignedOut
        }
    }

    pub fn account_commands(&self) -> Vec<ProviderCommand> {
        key_commands(self.key.read_optional().is_ok_and(|key| key.is_some()))
    }

    pub async fn administer(&self, command: &str, ui: &impl Interaction) -> Result<String, Error> {
        match command {
            SAVE => self.key.ask_in_terminal(&self.terminal).await,
            REMOVE => self.key.remove(ui, REMOVAL).await,
            _ => Err(Error::failed("Unknown OpenAI-compatible command")),
        }
    }
}
