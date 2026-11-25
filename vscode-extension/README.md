# Translation LSP

日本語: [README](https://github.com/hwtnb/translation-lsp/blob/main/README.ja.md) | [ドキュメント一覧](https://github.com/hwtnb/translation-lsp/tree/main/docs/ja)

Translation LSP translates code comments and Markdown and shows the translation in Hover; the source stays unchanged. It reads comments in more than 40 languages and file formats, including Rust, TypeScript, Python, Go, and shell scripts.

It translates with ChatGPT using your plan, the Anthropic API, the OpenAI API, or an OpenAI-compatible service such as Ollama. The DeepL API, Google Cloud Translation, and PLaMo 3 Translate also work. Until you select a provider, the extension sends no text to an external service.

The extension holds the language server for your system and starts it itself, so it needs nothing else to run.

## Set up a provider

1. Open the Settings editor, search for `Translation LSP`, and choose a provider in `Provider: Name`. `Target Language` is the language translations are written in, `ja` until you change it.
2. Choose `Restart` in the notification that follows. Changed settings apply when the language server restarts.
3. In a supported document, open the Code Actions and choose the item of your provider, such as `ChatGPT account…` or `Anthropic API key…`.

ChatGPT signs in through the browser. Saving an API key opens a terminal window of the OS; paste the key there and press Enter. Where no window opens, as in a remote window, run the command that the notification shows in a terminal on the machine that runs the server, as described under [When no terminal window opens](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/anthropic.md#when-no-terminal-window-opens). A service that needs no key, such as Ollama on your computer, needs only `Provider: Base Url` and `Provider: Model`.

Each provider's page describes its settings, creating the key, and cost:

- [ChatGPT](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/chatgpt.md)
- [Anthropic API](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/anthropic.md)
- [OpenAI API](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/openai-api.md)
- [OpenAI-compatible API](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/openai-compatible.md)
- [DeepL API](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/deepl.md)
- [Google Cloud Translation](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/google-translate.md)
- [PLaMo Translate](https://github.com/hwtnb/translation-lsp/blob/main/docs/providers/plamo-translate.md)

## Usage

- **Hover:** Hover over a comment to read its translation.
- **Markdown selection:** Select Markdown prose and keep the selection; the translation starts in the background. Hover over the selection to read it.
- **`Translate selection to …`:** This Code Action translates the selection at once.
- **CodeLens:** The first line of a translation shows above the comment. Click it to read the translation, with buttons to copy, retry, and delete it. It shows about the first 100,000 characters; `Copy` holds all of it.
- **Word lookup:** Select a word to ask the model for its meanings and an example. The DeepL API, Google Cloud Translation, and PLaMo Translate translate the word instead.
- **`… and open in tab`:** In Markdown and Plain Text, this Code Action opens the translation of the selection, or of the whole document, in a separate document.

`Translation LSP: Restart Language Server` in the Command Palette restarts the server. How each operation works in VS Code is described under [VS Code](https://github.com/hwtnb/translation-lsp/blob/main/docs/usage.md#vs-code), and every setting under [VS Code settings](https://github.com/hwtnb/translation-lsp/blob/main/docs/configuration.md#vs-code-settings).

## Before you send text

- Merely passing over a comment can consume provider usage once VS Code sends a Hover request.
- If a comment contains a credential, the credential is sent with it.
- Selecting Markdown prose just to copy it can also send it for translation, unless you turn off `Features: Automatic Selection`.
- Looking up a word also sends the line it is on.

What each request contains is described under [What is sent](https://github.com/hwtnb/translation-lsp/blob/main/docs/usage.md#what-is-sent).

## Supported systems

The extension needs VS Code 1.123 or later. Its packages hold the server for these systems:

| OS | Versions | CPU |
| --- | --- | --- |
| macOS | macOS 12 or later | Apple silicon and Intel |
| Linux | Ubuntu 22.04 or later, Debian 12 or later, and other distributions with glibc 2.35 or later | ARM64 and x86-64 |
| Windows | Windows 10 or later on x86-64, Windows 11 on ARM64 | ARM64 and x86-64 |

There is no package for Alpine Linux or other Linux distributions with musl, nor for 32-bit ARM. The servers are not signed with Authenticode and not notarized by Apple.

In a remote window, such as over SSH, in a container, or in WSL, the extension and its server run on the remote machine by default, and VS Code installs the package for the remote machine's system. The table above applies to that machine.

To start a server that you built, set `Server: Path` to its absolute path. The extension then starts that server in place of its own. On a system without a package, VS Code does not install the extension from the Marketplace. There, build the extension and install it from its VSIX file, as described under [VS Code extension](https://github.com/hwtnb/translation-lsp/blob/main/docs/development/README.md#vs-code-extension), and set `Server: Path` to a server that you [built from source](https://github.com/hwtnb/translation-lsp#build-the-server-from-source).

## Documentation

The [Translation LSP repository](https://github.com/hwtnb/translation-lsp) holds the documentation, which also covers the extension for Zed, and the source code. The extension and the server are under the MIT License; `THIRD_PARTY_LICENSES.txt` and `server/THIRD_PARTY_LICENSES.txt` in the extension list the licenses of the code they include.
