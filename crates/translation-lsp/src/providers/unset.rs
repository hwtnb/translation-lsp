//! The provider in use while the settings choose none. It translates nothing
//! and shows how to choose a provider.

use crate::store::Identity;

use super::{
    AccountChange, AuthUiState, Capabilities, ChoiceKind, Interaction, Notice, Output,
    ProviderCommand, Translated, TranslationProvider, error::Error,
};

macro_rules! menu {
    () => {
        "Set up Translation LSP…"
    };
}
const MENU: &str = menu!();

const OPEN_SETTINGS: ProviderCommand = ProviderCommand {
    command: "translation-lsp.open-settings",
    title: "Open Zed settings file",
    changes_account: AccountChange::Never,
};
const OPEN_GUIDE: ProviderCommand = ProviderCommand {
    command: "translation-lsp.open-setup-guide",
    title: "Open setup guide",
    changes_account: AccountChange::Never,
};
const COMMANDS: [ProviderCommand; 2] = [OPEN_SETTINGS, OPEN_GUIDE];
/// The title of [`OPEN_SETTINGS`] for an editor that opens its own settings.
const OPEN_OWN_SETTINGS: &str = "Open settings";

/// The message of the prompt shown when the server starts.
const PROMPT: &str =
    "Translation LSP has no provider and translates nothing. Choose one in Zed's settings.";
/// [`PROMPT`] for an editor that opens its own settings.
const OWN_PROMPT: &str =
    "Translation LSP has no provider and translates nothing. Choose one in the settings.";
const NEXT: &str = "Add lsp.translation-lsp.initialization_options.provider to Zed's settings, \
    save them, then run editor: restart language server.";
/// [`NEXT`] for an editor that opens its own settings.
const OWN_NEXT: &str = "Choose a provider in the settings, then restart the language server.";
const NOT_SET_UP: &str = concat!(
    "Translation LSP has no provider. Run \"",
    menu!(),
    "\" from the code actions to choose one"
);
/// The README of this version.
const README: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/v",
    env!("CARGO_PKG_VERSION"),
    "/README.md"
);
/// The section of the README that sets up a provider in Zed's settings.
const GUIDE: &str = "set-up-a-provider";
/// The section of the README that sets up VS Code, the editor that opens
/// settings of its own.
const OWN_GUIDE: &str = "install-in-vs-code";

/// The context of the provider that translates nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnsetContext;

#[derive(Clone, Default)]
pub(crate) struct UnsetProvider {
    /// Whether the editor opens its own settings, so that no text names Zed.
    own_settings: bool,
}

impl UnsetProvider {
    pub fn new(own_settings: bool) -> Self {
        Self { own_settings }
    }

    /// Asks the user to choose a provider, and runs the command they pick.
    /// Gives the message to show after it, or none if they picked nothing.
    pub async fn prompt(&self, ui: &impl Interaction) -> Result<Option<String>, Error> {
        let commands = self.account_commands();
        let titles: Vec<_> = commands
            .iter()
            .map(|command| command.title.to_owned())
            .collect();
        let prompt = if self.own_settings {
            OWN_PROMPT
        } else {
            PROMPT
        };
        let Some(command) = ui
            .choose(ChoiceKind::Message, prompt, &titles)
            .await?
            .and_then(|index| commands.get(index))
        else {
            return Ok(None);
        };
        self.administer(command.command, ui).await.map(Some)
    }
}

impl TranslationProvider for UnsetProvider {
    type Context = UnsetContext;

    fn identity(&self, _output: Output) -> Identity {
        Identity::new("unset")
    }

    fn context(&self) -> Result<UnsetContext, Error> {
        Err(Error::configuration(NOT_SET_UP))
    }

    fn auth_ui_state(&self) -> AuthUiState {
        AuthUiState::SignedOut
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            server_name: "Translation LSP",
            automatic_selection: false,
            account_menu: Some(MENU),
            commands: &COMMANDS,
            hover_note: None,
            comment_notice: Some(Notice {
                title: "Translation LSP is not set up",
                body: concat!("To choose a provider, run ", menu!(), " from Code Actions."),
            }),
            range_output_bounded: false,
        }
    }

    fn account_commands(&self) -> Vec<ProviderCommand> {
        COMMANDS
            .map(|command| {
                if self.own_settings && command.command == OPEN_SETTINGS.command {
                    ProviderCommand {
                        title: OPEN_OWN_SETTINGS,
                        ..command
                    }
                } else {
                    command
                }
            })
            .to_vec()
    }

    async fn translate(
        &self,
        _intent: super::Intent,
        _output: Output,
        _text: &str,
        _target_language: &str,
        _context: UnsetContext,
    ) -> Result<Translated<UnsetContext>, Error> {
        Err(Error::configuration(NOT_SET_UP))
    }

    async fn administer(&self, command: &str, ui: &impl Interaction) -> Result<String, Error> {
        if command == OPEN_SETTINGS.command {
            ui.open_settings().await?;
        } else {
            let section = if self.own_settings { OWN_GUIDE } else { GUIDE };
            ui.open(&format!("{README}#{section}")).await?;
        }
        Ok(if self.own_settings { OWN_NEXT } else { NEXT }.into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// An editor that opens every page and records it.
    #[derive(Default)]
    struct Browser(Mutex<Vec<String>>);

    impl Interaction for Browser {
        async fn open(&self, uri: &str) -> Result<(), Error> {
            self.0.lock().unwrap().push(uri.to_owned());
            Ok(())
        }

        async fn open_settings(&self) -> Result<(), Error> {
            unreachable!()
        }

        async fn choose(
            &self,
            _kind: ChoiceKind,
            _message: &str,
            _choices: &[String],
        ) -> Result<Option<usize>, Error> {
            unreachable!()
        }
    }

    /// The anchor that GitHub gives a heading.
    fn anchor(heading: &str) -> String {
        heading
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
            .map(|c| if c == ' ' { '-' } else { c })
            .collect()
    }

    fn headings(readme: &str) -> Vec<&str> {
        readme
            .lines()
            .filter_map(|line| {
                line.trim_start_matches('#')
                    .strip_prefix(' ')
                    .filter(|_| line.starts_with('#'))
            })
            .collect()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_guide_opens_the_section_for_the_editor() {
        for (own_settings, section) in [(false, GUIDE), (true, OWN_GUIDE)] {
            let browser = Browser::default();
            UnsetProvider::new(own_settings)
                .administer(OPEN_GUIDE.command, &browser)
                .await
                .unwrap();
            assert_eq!(*browser.0.lock().unwrap(), [format!("{README}#{section}")]);
        }
    }

    /// Both sections are in the README, and the Japanese README has them at
    /// the same place, under its own names.
    #[test]
    fn the_sections_of_the_guide_are_in_both_readmes() {
        let english = headings(include_str!("../../../../README.md"));
        let japanese = headings(include_str!("../../../../README.ja.md"));
        assert_eq!(english.len(), japanese.len());
        for (section, translated) in [
            (GUIDE, "プロバイダーの設定"),
            (OWN_GUIDE, "VS Codeに入れる"),
        ] {
            let index = english
                .iter()
                .position(|heading| anchor(heading) == section)
                .unwrap_or_else(|| panic!("{section}"));
            assert_eq!(japanese[index], translated);
        }
    }
}
