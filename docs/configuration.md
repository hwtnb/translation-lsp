# Configuration

[日本語](ja/configuration.md)

This page describes the settings under `lsp.translation-lsp`. The fields of each provider are listed on the [ChatGPT](providers/chatgpt.md), [Anthropic API](providers/anthropic.md), [OpenAI API](providers/openai-api.md), [OpenAI-compatible API](providers/openai-compatible.md), [DeepL API](providers/deepl.md), and [Google Cloud Translation](providers/google-translate.md) pages.

## Full example

The extension ID, language server settings key, and native executable name are `translation-lsp`.

Add the following entry to `lsp` in your existing Zed settings. Choose a model available to the account you will connect; the model and reasoning effort below are examples.

```json
{
  "lsp": {
    "translation-lsp": {
      "initialization_options": {
        "target_language": "ja",
        "source_language": "auto",
        "hover_timeout_ms": 20000,
        "selection_timeout_ms": 30000,
        "document_timeout_ms": 180000,
        "selection_debounce_ms": 450,
        "comment_parse_timeout_ms": 1000,
        "features": {
          "comment_hover": true,
          "automatic_selection": true,
          "selection_action": true,
          "document_action": true,
          "comment_lens": true
        },
        "hover": {
          "header": true,
          "format": "markdown"
        },
        "translation_limits": {
          "input_kib": {
            "hover": 16,
            "document": 1024
          },
          "output_kib": 4096
        },
        "document_split": {
          "enabled": false,
          "parallel": 4,
          "chunk_kib": 8
        },
        "disk_cache": {
          "enabled": false,
          "days": 30
        },
        "provider": {
          "name": "chatgpt",
          "model": "gpt-6.1-sol",
          "effort": "low"
        }
      }
    }
  }
}
```

## Server path

### Lookup order

The extension starts the first server it finds, in this order:

1. The executable that `binary.path` names.
2. `translation-lsp` on the worktree's PATH.
3. The server the extension downloaded.

`binary.arguments` and `binary.env` apply to the server from any of the three.

### Automatic download and cache

When the first two give no server, the extension downloads one from this repository's [GitHub Releases](https://github.com/hwtnb/zed-translation-lsp/releases) and keeps it in its work directory. It downloads the server of its own version, which is not always the latest release. Later starts use the kept server and need no network access. After the extension is updated, it downloads the server of its new version and removes the servers of other versions.

A download that does not complete is discarded, and the next start tries again. The extension never starts a partly downloaded server. The download goes through Zed's HTTPS downloader; the extension does not compare the archive with the release's `SHA256SUMS`.

A server on PATH comes before the downloaded one, so updating the extension does not replace it.

### Manual installation

Set `binary.path` when the extension cannot reach GitHub, when no server is published for your system, or to use a server you built. Get the server in one of two ways:

- Build it as described under [Build the server from source](../README.md#build-the-server-from-source).
- Download the archive for your system from the release of the extension's version and extract it. Keep the license files that the archive contains next to the executable.

To check a downloaded archive, compare its SHA-256 with its line in the release's `SHA256SUMS`: use `sha256sum` on Linux, `shasum -a 256` on macOS, or `Get-FileHash -Algorithm SHA256` in PowerShell.

`binary.path` is an absolute path for the OS where the server runs, including `.exe` on Windows. For example:

| OS | Example `binary.path` value |
| --- | --- |
| Windows | `C:/Users/you/src/translation-lsp/target/release/translation-lsp.exe` |
| macOS | `/Users/you/src/translation-lsp/target/release/translation-lsp` |
| Linux | `/home/you/src/translation-lsp/target/release/translation-lsp` |

Windows paths can use forward slashes in JSON. If you use backslashes, escape each one as `\\`. Do not include extra shell quotes inside the path value, even when the path contains spaces.

### Language server lists

If you explicitly configure a language's `language_servers`, add `"translation-lsp"` to the existing array. The settings of each language are listed under [Language server settings](languages.md#language-server-settings).

To turn this extension off for one language, put `"!translation-lsp"` in that language's `language_servers`, followed by `"..."` to keep the other servers:

```json
{
  "languages": {
    "YAML": {
      "language_servers": ["!translation-lsp", "..."]
    }
  }
}
```

In that language, neither Hover translation nor the Code Actions appear. To turn off only some features in a language, use [`features`](#features).

## Translation languages

UI labels are in English regardless of `target_language`. `target_language` defaults to `ja`; `source_language` defaults to `auto`. With `auto`, the same translation request identifies the source language.

A value that is an ISO 639 language code, alone or with subtags such as `zh-Hans` or `pt-BR`, is shown with the English name of its language, as `Japanese (ja)`, both in Code Action titles and in the instructions sent to the provider. Any other value, such as `Japanese`, is used as written.

The DeepL API and Google Cloud Translation take no instructions and only language codes; see [Settings](providers/deepl.md#settings) of the DeepL API page and [Settings](providers/google-translate.md#settings) of the Google Cloud Translation page.

## Features

`features` turns each way of starting a translation, and the CodeLens line on comments, on or off, for every language or for one language. Every value is a boolean.

| Setting inside `features` | What it turns on or off | Language IDs |
| --- | --- | --- |
| `comment_hover` | Starting a translation by Hover over a comment. | `rust`, `javascript`, `typescript`, `typescriptreact`, `csharp`, `c`, `cpp`, `python`, `go`, `java`, `kotlin`, `swift`, `ruby`, `php`, `html`, `vue`, `svelte`, `astro`, `xml`, `css`, `scss`, `sql`, `lua`, `zig`, `proto`, `graphql`, `clojure`, `haskell`, `ocaml`, `ocaml.interface`, `scala`, `dart`, `elixir`, `terraform`, `terraform-vars`, `hcl`, `nix`, `prisma`, `gleam`, `erlang`, `cmake`, `glsl`, `json`, `jsonc`, `yaml`, `toml`, `shellscript`, `powershell`, `dockerfile`, `dockercompose`, `makefile` |
| `automatic_selection` | Starting a translation after a stable Markdown selection. | `markdown` |
| `selection_action` | The `Translate selection to …` Code Action. | Every language ID in the table below |
| `document_action` | The `Translate selection to … and open in tab` and `Translate document to … and open in tab` Code Actions. | `markdown`, `plaintext` |
| `comment_lens` | The [CodeLens](usage.md#codelens) line on a comment with no translation, failure, or translation in progress. | `rust`, `javascript`, `typescript`, `typescriptreact`, `csharp`, `c`, `cpp`, `python`, `go`, `java`, `kotlin`, `swift`, `ruby`, `php`, `html`, `vue`, `svelte`, `astro`, `xml`, `css`, `scss`, `sql`, `lua`, `zig`, `proto`, `graphql`, `clojure`, `haskell`, `ocaml`, `ocaml.interface`, `scala`, `dart`, `elixir`, `terraform`, `terraform-vars`, `hcl`, `nix`, `prisma`, `gleam`, `erlang`, `cmake`, `glsl`, `json`, `jsonc`, `yaml`, `toml`, `shellscript`, `powershell`, `dockerfile`, `dockercompose`, `makefile` |

`features`, each setting inside it, `languages`, each language inside `languages`, and each setting of a language can be omitted. For a feature in a language, the value under `languages.<language ID>` applies. Without it, the value written directly inside `features` applies. Without that either, the feature is on.

The following example turns tab translations off in every language, and comment Hover off in YAML and TOML:

```json
"features": {
  "comment_hover": true,
  "automatic_selection": true,
  "selection_action": true,
  "document_action": false,
  "languages": {
    "yaml": { "comment_hover": false },
    "toml": { "comment_hover": false }
  }
}
```

Where `comment_hover` is off, Hover over a comment starts no translation. A translation started by `Translate selection to …` still appears on Hover. Where `selection_action` or `document_action` is off, its Code Actions are not listed. The cancel, retry, and account menu Code Actions are not affected. Where `comment_lens` is off, the server menu moves above the first line of the file.

The keys of `languages` are the following language IDs, in lowercase:

| Zed language | Language ID |
| --- | --- |
| Rust | `rust` |
| JavaScript, including `.jsx` files | `javascript` |
| TypeScript | `typescript` |
| TSX (`.tsx` files) | `typescriptreact` |
| CSharp | `csharp` |
| C | `c` |
| C++ | `cpp` |
| Python | `python` |
| Go | `go` |
| Java | `java` |
| Kotlin | `kotlin` |
| Swift | `swift` |
| Ruby | `ruby` |
| PHP | `php` |
| HTML | `html` |
| Vue.js | `vue` |
| Svelte | `svelte` |
| Astro | `astro` |
| XML | `xml` |
| CSS | `css` |
| SCSS | `scss` |
| SQL | `sql` |
| Lua | `lua` |
| Zig | `zig` |
| Proto | `proto` |
| GraphQL | `graphql` |
| Clojure | `clojure` |
| Haskell | `haskell` |
| OCaml | `ocaml` |
| OCaml Interface | `ocaml.interface` |
| Scala | `scala` |
| Dart | `dart` |
| Elixir | `elixir` |
| Terraform | `terraform` |
| Terraform Vars | `terraform-vars` |
| HCL | `hcl` |
| Nix | `nix` |
| Prisma | `prisma` |
| Gleam | `gleam` |
| Erlang | `erlang` |
| CMake | `cmake` |
| GLSL | `glsl` |
| JSON | `json` |
| JSONC | `jsonc` |
| YAML | `yaml` |
| TOML | `toml` |
| Shell Script | `shellscript` |
| PowerShell | `powershell` |
| Dockerfile | `dockerfile` |
| Docker Compose | `dockercompose` |
| Make | `makefile` |
| Markdown | `markdown` |
| Plain Text | `plaintext` |

Any other key of `languages`, such as `javascriptreact`, is a settings error. So is a feature written for a language outside its language IDs, such as `languages.rust.document_action`.

## Hover

`hover` sets how Hover shows a translation. `hover` and each setting inside it can be omitted.

| Setting inside `hover` | Default | Allowed values | Effect |
| --- | --- | --- | --- |
| `header` | `true` | `true`, `false` | With `false`, Hover shows a translation without the title `Translation LSP`, the provider's note, and the rule below them. Hover still titles a failure. |
| `format` | `"markdown"` | `"markdown"`, `"text"` | `"markdown"` shows a translation with the formatting it writes and other characters as written. `"text"` shows it in a code block as written. A dictionary entry shows formatted with both. See [How Hover shows a translation](usage.md#how-hover-shows-a-translation). |

Any other setting inside `hover`, or any other value, is a settings error.

## Document split

`document_split` sends a translation opened in a tab in parts, several at once, and opens the translation when every part is done. `document_split` and each setting inside it can be omitted. Without `enabled` set to `true`, a translation opened in a tab sends its text as one whole.

```json
"document_split": {
  "enabled": true,
  "parallel": 4,
  "chunk_kib": 8
}
```

| Setting inside `document_split` | Default | Allowed values | Effect |
| --- | --- | --- | --- |
| `enabled` | `false` | `true`, `false` | With `true`, `Translate document to … and open in tab` and `Translate selection to … and open in tab` send a text of more than one and a half times `chunk_kib` in parts. With `false`, the other settings do nothing, though their values are still checked. |
| `parallel` | `4` | Integer `1`–`8`, inclusive | The requests that one translation in a tab sends at once. |
| `chunk_kib` | `8` | Integer `1`–`16384`, inclusive | The size, in KiB, that a part aims at. |

Any other setting inside `document_split`, or any other value, is a settings error.

A text is divided as follows:

- Markdown is divided before a heading where it can be, so that a part holds whole sections up to about `chunk_kib`. Only a section of more than twice `chunk_kib` is divided between its own paragraphs, lists, tables, and code blocks.
- A heading stays with the block after it, and a list, quote, table, code block, or HTML block is never divided, so a part can exceed `chunk_kib`.
- Plain Text is divided between paragraphs, at blank lines.
- A part without a letter or a digit, such as a thematic break alone, is not sent and stays as written.

The translations of the parts are joined in the order of the document, with the line breaks between the parts as written.

- `document_timeout_ms` covers all the parts together.
- `translation_limits.output_kib` applies to the joined translation: once the parts that completed exceed it, the translation fails and no more parts are sent.
- If any part fails, or its translation comes back blank, nothing opens, and the requests still in progress stop.

With `document_split` enabled, a tab translation and a range translation are separate operations, as with `provider.document`; see [Admission and execution slots](behavior.md#admission-and-execution-slots). What each request holds is described under [What is sent](usage.md#what-is-sent).

Limitations:

- The parts are translated separately, so one term can be translated in different words in different parts.
- If the model returns a part untranslated, the server does not detect it.

## Disk cache

`disk_cache` keeps completed translations on disk as well as in memory, so that they are used again after the language server restarts. `disk_cache` and each setting inside it can be omitted.

```json
"disk_cache": {
  "enabled": true,
  "days": 30
}
```

| Setting inside `disk_cache` | Default | Allowed values | Effect |
| --- | --- | --- | --- |
| `enabled` | `false` | `true`, `false` | With `true`, each completed translation is also written to disk, and a translation found there is shown without sending the text again, after a restart too. With `false`, translations are kept in memory only, and `days` does nothing, though its value is still checked. |
| `days` | `30` | Integer `1`–`365`, inclusive | A translation on disk that has not been used for this many days is removed. |

Any other setting inside `disk_cache`, or any other value, is a settings error.

The translations are kept in the folder `translation-lsp/translations` inside the cache folder of the OS, apart from the folder of API keys:

| OS | Folder |
| --- | --- |
| macOS | `~/Library/Caches/translation-lsp/translations` |
| Linux | `$XDG_CACHE_HOME/translation-lsp/translations`, or `~/.cache/translation-lsp/translations` when `XDG_CACHE_HOME` is unset |
| Windows | `%LOCALAPPDATA%\translation-lsp\translations` |

Each file holds one translation, one part of a translation sent in parts, one dictionary entry with its CodeLens line, or the result that an automatic request found the text already in the target language. Its name is a SHA-256 hash of what was sent and the settings that change the translation.

- The text that was sent and the lines around it are not written. A dictionary entry holds its word and example sentence, and a translation holds what was left untranslated, such as code and identifiers.
- The files are not encrypted. On macOS and Linux, the folder and each file are open to their owner alone.
- Failed, incomplete, canceled, and timed-out results are not written.
- When the folder cannot be used, translations are kept in memory only, and the server writes one line to its log for each kind of failure, without the text or the translation.

The server cleans the folder after it starts and then every six hours, apart from translation requests. It removes the translations not used for longer than `days`, then the least recently used ones while the rest take more than 64 MiB; the 64 MiB limit is fixed. A translation is used when a request finds it, in memory or on disk. Files of other names in the folder stay.

Turning `disk_cache` off leaves the files in place. To remove them, choose `Delete all cached translations`, which removes every translation in the folder, including those of other providers and models, even with `disk_cache` off, or remove the folder. With `disk_cache` on, the other deletion commands also remove the files of what they delete; while it is off, they leave the files. See [Deleting cached translations](usage.md#deleting-cached-translations).

## Timing

Translation timing options are optional:

| Setting | Default | Allowed values | Effect |
| --- | --- | --- | --- |
| `hover_timeout_ms` | `20000` | Integer `1000`–`120000`, inclusive | Response deadline from receipt, including queue time, for each Hover request. |
| `selection_timeout_ms` | `30000` | Integer `1000`–`120000`, inclusive | Execution budget from dispatch for retained manual selections and Markdown background translations. |
| `document_timeout_ms` | `180000` | Integer `1000`–`600000`, inclusive | Execution budget from dispatch for full-document or selection translations opened in a tab; queue time is additional. |
| `selection_debounce_ms` | `450` | Integer `100`–`2000`, inclusive | Native server's wait after observing a new selection. Zed's own observation delay is additional. |
| `comment_parse_timeout_ms` | `1000` | Integer `100`–`60000`, inclusive | Limit on reading the comments of a C, C++, C#, or Python document, from the start of the reading. See [Documents whose comments cannot be read](languages.md#documents-whose-comments-cannot-be-read). |

## Size limits

Translation size limits belong to `initialization_options.translation_limits`. Values are positive integers in KiB (1 KiB = 1024 UTF-8 bytes), not character or token counts. Omitted objects and fields use their own defaults.

| Setting inside `translation_limits` | Default | Applies to |
| --- | --- | --- |
| `input_kib.hover` | `16` | New comment Hover requests, Markdown background translation, and manual selection translations shown in Hover. |
| `input_kib.document` | `1024` | Whole documents and selections translated into a separate tab. |
| `output_kib` | `4096` | Completed translations, regardless of display destination. |

For example, `"translation_limits": {"input_kib": {"document": 2048}}` changes only the document input limit to 2 MiB. Zero, negative, fractional, null, unknown, and nonnumeric values are rejected, as are sizes that cannot be represented on the server platform. Restart the language server after editing these settings.

Explicit requests over the input limit report the size, limit, and setting name; oversized automatic selections are skipped. Existing cached translations can still be read without another translation request.

## Work and cache limits

Work and cache limits are optional objects inside `translation_limits`:

```json
"translation_limits": {
  "work": { "admitted": 8, "concurrent": 2, "documents": 1, "automatic": 3, "hover_waiters": 16 },
  "cache": { "entries": 64, "capacity_kib": 16384 }
}
```

Each count is a positive integer:

- `admitted`: running and queued translations, including canceled ones until their workers end
- `concurrent`: translations running at once
- `documents`: pieces of work started as tab translations at once
- `automatic`: translations kept by comment Hover or automatic selection without a manual or tab request
- `hover_waiters`: Hover responses waiting at once

The counts must satisfy `documents <= concurrent <= admitted` and `automatic < admitted`. This leaves room to accept explicit requests, but does not keep a worker idle. Increasing concurrency can increase simultaneous provider usage.

`cache.entries` bounds the combined successful, failed, and tab cache entries across all documents. `cache.capacity_kib` bounds their stored text, not total process memory. The effective capacity is the greater of this value and `output_kib`, so one maximum-size result can fit. The translations kept in memory by what was sent, described under [Cache](behavior.md#cache), take the same two limits of their own.

Metadata, source documents, in-flight input, and temporary tab files are outside this text budget. Defaults are a starting point for sequential reading, not measured provider or hardware limits. Omitted nested fields retain their defaults; restart the language server after changing them.

## Maximum values

The following are **maximum configurable values**, separate from the defaults above. Larger values fail initialization rather than being silently clamped.

| Setting inside `translation_limits` | Maximum |
| --- | --- |
| `input_kib.hover` | `1024` (1 MiB) |
| `input_kib.document`, `output_kib` | `16384` (16 MiB each) |
| `work.admitted` | `64` |
| `work.concurrent`, `work.documents` | `8` each |
| `work.automatic` | `63`, also less than `admitted` |
| `work.hover_waiters` | `256` |
| `cache.entries` | `4096` |
| `cache.capacity_kib` | `262144` (256 MiB) |

| Setting inside `document_split` | Maximum |
| --- | --- |
| `parallel` | `8` |
| `chunk_kib` | `16384` (16 MiB) |

| Setting inside `disk_cache` | Maximum |
| --- | --- |
| `days` | `365` |

Two combinations are also limited. They are checked for the configured worst case, even when the queue is empty:

- `max(input_kib.hover, input_kib.document) × work.admitted` must not exceed `262144` KiB (256 MiB). For example, 16 MiB document input with 64 accepted jobs is rejected; use at most 16 accepted jobs at that input limit.
- `output_kib × work.concurrent` must not exceed `65536` KiB (64 MiB). A 16 MiB output limit permits at most four simultaneous translations.

These ceilings are a local resource policy, not measured hardware limits or provider guarantees. Source documents, parser state, allocator overhead, transport buffers, and temporary translation files are additional costs, so these figures are **not a process-memory guarantee**. A limit beyond these ceilings needs a change in the server; Zed settings cannot raise them. The reasons for the ceilings are described under [Size limit ceilings](development/README.md#size-limit-ceilings).

Increasing a local limit does not increase the model's context or output capacity or extend the request deadline. A translation opened in a tab sends its text as one whole unless [`document_split`](#document-split) is enabled. Failed, incomplete, and oversized results are not displayed or saved as completed translations.

## Settings errors and restart

Unknown fields, wrong value types, and out-of-range values prevent the language server from starting and return an English settings error. Omitting a field uses its default. Without `provider`, the language server translates nothing and asks you to choose a provider, as described under [Set up a provider](../README.md#set-up-a-provider).

Restart the language server after changing these settings. Settings changes do not apply to a running server. See [Zed's language server settings](https://zed.dev/docs/configuring-languages#configuring-language-servers).

## Changing the translation model

1. Open Zed's Command Palette and run `zed: open settings file`.
2. Edit `lsp.translation-lsp.initialization_options.provider.model` with the model slug you want to use.
3. Set the provider's other fields if needed. They are listed on the [ChatGPT](providers/chatgpt.md#settings), [Anthropic API](providers/anthropic.md#settings), [OpenAI API](providers/openai-api.md#settings), and [OpenAI-compatible API](providers/openai-compatible.md#settings) pages. The DeepL API and Google Cloud Translation have no models to choose.
4. Save the settings. In a document using this extension, run `editor: restart language server` from the Command Palette.

The extension sends the configured model slug to the translation API without substituting another model. The API determines whether that model, effort, and speed are available to your account.

Translation requests use ordinary text output. Choose a model available to your ChatGPT account, your API key, or the service; the extension does not switch to another model when a request fails.

### Tab translations with another model

To use a different model only for translations opened in a tab, add `provider.document`. `Translate selection to … and open in tab` and `Translate document to … and open in tab` then send the model and options written there. Comment Hover, automatic selection, manual selection translation, and retries keep using the surrounding `provider` values.

`provider.document.model` is required. The other values inside `provider.document` are optional and are not inherited from the surrounding settings: an omitted value takes its own default, whatever is written outside.

`provider.document` cannot select another provider. Setting it also keeps tab translations separate from range translations, as described under [Admission and execution slots](behavior.md#admission-and-execution-slots).

The values that `provider.document` accepts, and an example, are on each provider's page: [ChatGPT](providers/chatgpt.md#tab-translations-with-another-model), [Anthropic API](providers/anthropic.md#tab-translations-with-another-model), [OpenAI API](providers/openai-api.md#tab-translations-with-another-model), and [OpenAI-compatible API](providers/openai-compatible.md#tab-translations-with-another-model).
