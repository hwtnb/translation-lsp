# Development

[日本語](../ja/development/README.md)

This page is for working on the extension and the language server, with topics such as the checks to run, the mock provider and its samples, CI and releases, and the dependency policy. How the server reads comments is described under [Comment readers](comment-readers.md).

## Checks

The crates use Rust Edition 2024, and the virtual workspace uses resolver 3.

- The native language server and `xtask` require Rust 1.98 or later. Dependency updates take priority over retaining an older minimum Rust version; update the affected requirement when needed.
- The WASM extension declares no `rust-version`. Cargo selects dependency versions for the whole workspace that suit the lowest `rust-version` among its members, so declaring the older compiler of the Zed registry there would hold back the dependencies of the native server. CI builds the extension with the registry's compiler instead.

Keep `Cargo.lock` in version control and use `--locked` for checks and release builds:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Mock provider

With `"provider": { "name": "mock" }` in the settings, the server uses the mock provider. It sends no text to an external service and returns fixed Japanese translations of the bundled samples, so the Zed flow can be checked without an account or an API key.

The mock provider supports only the comment samples and the samples marked Mock below, their newline variations, and the term `retry_count`. It returns an error for other text or target languages other than Japanese. The Hover header identifies its translations as `Mock translation (fixed sample)`.

### Comment samples

Hover over the comments in these samples.

| Languages | Samples |
| --- | --- |
| Rust | [samples/comment.rs](../../samples/comment.rs). With the menu of the comment's `Settings` left open, Hover over the comment still shows its translation. |
| C# | [samples/comment.cs](../../samples/comment.cs), with ordinary and XML documentation comments |
| C, C++ | [samples/comment.c](../../samples/comment.c) and [samples/comment.cpp](../../samples/comment.cpp), including Doxygen comments |
| JavaScript, JSX | [samples/comment.js](../../samples/comment.js) and [samples/comment.jsx](../../samples/comment.jsx) |
| TypeScript, TSX | [samples/comment.ts](../../samples/comment.ts) and [samples/comment.tsx](../../samples/comment.tsx) |
| Python | [samples/comment.py](../../samples/comment.py), with comments and docstrings |
| Go, Java, Kotlin | [samples/comment.go](../../samples/comment.go), [samples/comment.java](../../samples/comment.java), and [samples/comment.kt](../../samples/comment.kt) |
| Swift, Ruby | [samples/comment.swift](../../samples/comment.swift) and [samples/comment.rb](../../samples/comment.rb) |
| PHP, SQL | [samples/comment.php](../../samples/comment.php) and [samples/comment.sql](../../samples/comment.sql) |
| HTML | [samples/comment.html](../../samples/comment.html), with an HTML comment and comments in a script and a style sheet |
| CSS, SCSS | [samples/comment.css](../../samples/comment.css) and [samples/comment.scss](../../samples/comment.scss) |
| Vue.js, Svelte, Astro | [samples/comment.vue](../../samples/comment.vue), [samples/comment.svelte](../../samples/comment.svelte), and [samples/comment.astro](../../samples/comment.astro) |
| XML, Lua, Zig | [samples/comment.xml](../../samples/comment.xml), [samples/comment.lua](../../samples/comment.lua), and [samples/comment.zig](../../samples/comment.zig) |
| Protocol Buffers, GraphQL, makefile | [samples/comment.proto](../../samples/comment.proto), [samples/comment.graphql](../../samples/comment.graphql), and [samples/Makefile](../../samples/Makefile) |
| Clojure, Haskell | [samples/comment.clj](../../samples/comment.clj) and [samples/comment.hs](../../samples/comment.hs) |
| OCaml | [samples/comment.ml](../../samples/comment.ml) and [samples/comment.mli](../../samples/comment.mli) |
| Scala, Dart | [samples/comment.scala](../../samples/comment.scala) and [samples/comment.dart](../../samples/comment.dart) |
| Elixir | [samples/comment.ex](../../samples/comment.ex), with comments and docs |
| Terraform, Terraform Vars, HCL | [samples/comment.tf](../../samples/comment.tf), [samples/comment.tfvars](../../samples/comment.tfvars), and [samples/comment.hcl](../../samples/comment.hcl) |
| Nix | [samples/comment.nix](../../samples/comment.nix) |
| Prisma | [samples/comment.prisma](../../samples/comment.prisma) |
| Gleam | [samples/comment.gleam](../../samples/comment.gleam) |
| Erlang | [samples/comment.erl](../../samples/comment.erl) |
| CMake | [samples/comment.cmake](../../samples/comment.cmake) |
| GLSL | [samples/comment.frag](../../samples/comment.frag) |
| F# | [samples/comment.fsx](../../samples/comment.fsx) |
| Lean | [samples/comment.lean](../../samples/comment.lean) |
| JSON, JSONC | [samples/comment.json](../../samples/comment.json) and [samples/comment.jsonc](../../samples/comment.jsonc). Zed's JSON language server reports the comments in `comment.json`, and Hover translates them all the same. |
| YAML, TOML | [samples/comment.yaml](../../samples/comment.yaml) and [samples/comment.toml](../../samples/comment.toml) |
| Shell script, PowerShell | [samples/comment.sh](../../samples/comment.sh) and [samples/comment.ps1](../../samples/comment.ps1) |
| Dockerfile, Docker Compose | [samples/Dockerfile](../../samples/Dockerfile) and [samples/compose.yaml](../../samples/compose.yaml) |

### Other samples

| Sample | Provider | What to check |
| --- | --- | --- |
| [samples/hover.rs](../../samples/hover.rs), [samples/hover.java](../../samples/hover.java), [samples/hover.py](../../samples/hover.py) | Mock | How Hover shows headings, lists, tables, code, and URLs of a translation. `hover.java` has Javadoc comments, and `hover.py` a docstring with sections. The comment of `report` in `hover.rs` has `--`, quotation marks, and `...`, which the card of its CodeLens also shows as written. |
| [samples/paragraph.md](../../samples/paragraph.md) | Mock | Select the paragraph, run `Translate selection to Japanese (ja)` from Code Actions, then Hover over the selection. To see a dictionary entry, select only `retry_count` and run the same action. The mock provider requires manual execution for Markdown. |
| [samples/document.txt](../../samples/document.txt) | Mock and real | A Plain Text sample. The first paragraph is that of `paragraph.md`, for the mock provider's selection translation and dictionary entry. The two paragraphs after it are for a translation in a tab with a real provider. |
| [samples/document.md](../../samples/document.md) | Real | A translation opened in a tab keeps its Markdown, and selecting only code or only a link destination starts no automatic translation. The sample holds headings, lists, a table, a code block, inline code, emphasis, and three kinds of links: to an `https` URL, to a heading of the document, and to a file in the same folder. |
| [samples/languages.rs](../../samples/languages.rs) | Real | Which comments Hover leaves untranslated because they are already in the target language. See [Text already in the target language](#text-already-in-the-target-language). |
| [samples/languages.md](../../samples/languages.md) | Real | With `features.automatic_selection` set to `true` and `target_language` set to `ja`, selecting either Japanese paragraph shows no Hover, while selecting the English paragraph does. |
| [samples/long-document.md](../../samples/long-document.md) | Mock | A translation sent in parts. See [Translations sent in parts](#translations-sent-in-parts). |
| [samples/context-limit.md](../../samples/context-limit.md) | Real | A long document for the context of a model on your computer, which differs between computers and can be changed. With `plamo_translate` and a context smaller than the document, `Translate document to Japanese (ja) and open in tab` reports the limit of the context. With `document_split` enabled, it opens the translation when each part fits. See [Long texts](../providers/plamo-translate.md#long-texts). |
| [samples/repeated.rs](../../samples/repeated.rs) | Mock and real | Translations shown again without a request. See [Repeated translations](#repeated-translations). |
| [samples/comment.rs](../../samples/comment.rs) | Mock, Anthropic API | In VS Code, `Settings` after the comment opens the menu as a list at the top of the window, titled `Automatic translation: On (comment Hover and automatic selection translation)`. Choosing `Turn off automatic translation` makes the next menu say `Off`. After a translation, `Delete all cached translations` asks in a modal dialog first, and `Cancel` keeps them. With `anthropic_api`, `Anthropic API key…` from Code Actions opens a menu titled `Anthropic API key`. `Save Anthropic API key` opens a terminal that asks for the key; with a key saved, `Remove Anthropic API key` asks in a modal dialog first. Neither calls the API. |
| [samples/icons.rs](../../samples/icons.rs) | Mock and real | In VS Code, `$(sync)` and `$(file)` in a translation show as written in Hover, in its CodeLens line, and in the text opened from that line, also in code. The mock translation starts with `$(sync)`. |

The mock provider does not translate the samples marked only Real.

### Text already in the target language

[samples/languages.rs](../../samples/languages.rs) holds comments of these kinds:

- in English, in Japanese, and in Japanese with code and a URL
- half in English and half in Japanese
- in kanji only, and in Chinese (Simplified)
- in six sentences, with one English sentence among Japanese ones
- in Serbian written in Cyrillic: with `target_language` set to `sr-Latn`, Hover shows its translation in Latin script instead of leaving it as Serbian

### Translations sent in parts

With `document_split` set to `{"enabled": true, "chunk_kib": 1}`, `Translate document to Japanese (ja) and open in tab` sends [samples/long-document.md](../../samples/long-document.md) in four parts. One of them starts in the middle of the section `Closing connections`, whose code block holds a line that starts with `#`.

The mock provider translates it only when it is sent in parts. Sent as one, without `document_split` or with a `chunk_kib` too large to divide it, it is reported as not one of the bundled samples.

### Repeated translations

[samples/repeated.rs](../../samples/repeated.rs) holds the comment of `comment.rs` twice, and a documentation comment twice for a real provider. Hover over one, then the other: the second is shown without a request, as described under [Cache](../behavior.md#cache). With [`disk_cache`](../configuration.md#disk-cache) enabled, it is also shown without a request after Zed restarts.

With the mock provider and `disk_cache` enabled, the files in its folder show how many parts a tab sent anew; count them before and after. For example:

1. Set `document_split` as for `long-document.md`, and translate that sample in a tab.
2. Delete one item of its list, and translate it again.
3. One file is added, for the one part that changed, and the mock provider still translates the shorter part.

Files do not count requests otherwise: a retry replaces a file, and one translation of the DeepL API or Google Cloud Translation may take several requests.

### Arguments for the stdio tests

The stdio tests start the executable with `--mock-delay-ms <milliseconds>`, `--mock-fail`, `--mock-timeout-ms <milliseconds>`, or `--mock-same-language` to delay mock translations, make them fail, set their deadlines, or have automatic translations find the text already in the target language. These arguments affect only the mock provider.

## VS Code extension

`vscode-extension` holds the VS Code extension, written in TypeScript. It starts the server named in its settings and shows what the server sends with VS Code's own parts. Run these commands in that directory, with Node.js 22.12 or later:

```sh
npm ci
npm run check
npm run lint
npm run format:check
npm test
npm run package
```

`check` checks the types. `lint` runs Oxlint, including its rules that use the types, such as one against a promise that nothing awaits, and fails on any warning. `format:check` checks that Oxfmt leaves `src` and `test` as they are; `npm run format` formats them.

`test` uses Vitest to check the settings the extension sends, localized text, translation comment author names, and how the extension starts and stops the server. These tests run without VS Code. `package` builds the extension with Rolldown and writes `translation-lsp.vsix`.

The settings of the extension are listed in `package.json`, and the settings that each provider takes in `src/providers.json`. The server's tests read both: every server setting must have one VS Code setting with the server's default, and each provider must accept the settings of its row and refuse the others. They also check that `activationEvents` matches every language in `zed-extension/extension.toml` plus the mapped VS Code language IDs below.

To check the extension in VS Code, start VS Code with a user data directory and an extensions directory of their own, so that your usual settings and extensions stay apart:

```sh
code --user-data-dir <directory> --extensions-dir <directory> --install-extension translation-lsp.vsix
```

Start it again with the same two options and a folder of samples, set `translationLsp.server.path` to the server you built, and `translationLsp.provider.name` to `mock` in `settings.json`. The Settings editor does not list `mock`.

### Mapped VS Code language IDs

`Language::of_document` in `crates/translation-lsp/src/language.rs` maps the following VS Code language IDs to the registered language that reads their comments and supplies their `features.languages` settings. Settings do not accept these IDs as keys.

| VS Code language ID | Registered language |
| --- | --- |
| `javascriptreact` | `javascript` |
| `kotlinscript` | `kotlin` |
| `proto3` | `proto` |
| `terraform-stack` | `hcl` |
| `terraform-deploy` | `hcl` |
| `terraform-test` | `hcl` |
| `terraform-mock` | `hcl` |
| `terraform-search` | `hcl` |
| `terraform-policy` | `hcl` |
| `terraform-policytest` | `hcl` |

### Messages of the VS Code extension

The extension names the messages it takes under `capabilities.experimental.translationLsp` in its `initialize` request. The server uses each only when its value is `true`; a client that names none, such as Zed, gets the requests and texts of Zed.

| Name | What changes |
| --- | --- |
| `showTranslation` | `translation-lsp.show-translation` sends the notification `translation-lsp/showTranslation` in place of a card. The extension uses `Translation LSP: <language name>` as the comment author for both translations and errors. The name comes from `targetLanguage` and the VS Code display language, with the original value as a fallback. |
| `openSettings` | Opening the settings sends the request `translation-lsp/openSettings`, with `null` as its parameters, in place of opening Zed's settings file. A success means the settings opened. The texts that name Zed's settings name the settings instead. |
| `choose` | A menu that the user opens, such as `Settings` or the account menu of the provider, and the question before every cached translation is deleted or a saved key is removed, send the request `translation-lsp/choose` in place of a message with buttons. The prompt that the server opens itself when no provider is set stays a message. |
| `themeIcons` | The Hover header is `$(globe) Translation LSP`. In Hover and in `showTranslation`, the server puts a word joiner after each `$` before `(`, and makes no link to a URL or a `#section` with `$(`, so that VS Code draws no icon from a translation. In a CodeLens line, it puts a backslash before each icon name. |

For a client whose standard `general.markdown.parser` is `marked`, as VS Code's is, the server checks the translation of a Markdown document as Zed reads it and as pulldown-cmark reads it with only the extensions of marked: tables, strikethrough, and task lists. This comes close to marked, which has none of Zed's other extensions, such as footnotes and heading attributes.

Footnotes appear as text so that marked cannot turn them into links. For links outside the current document, both readings check the destination against the original. An unverified destination makes the whole translation a code block. This also applies to links that only marked recognizes, for example in metadata or heading attributes. A destination can be verified from a link elsewhere in the original.

For example, if the original contains `https://example.com` and both readings find that URL in the translated prose, it stays clickable. In other bare `://` sequences, `www.` prefixes, and email addresses, the server inserts a word joiner to prevent marked from creating links.

`translation-lsp/showTranslation` carries these fields:

| Field | Value |
| --- | --- |
| `uri`, `range` | Where the translation is now. |
| `version` | The version of the document that the translation belongs to. The extension shows no translation of another version. |
| `failed` | `true` when it shows why a translation failed. |
| `markdown` | The Markdown to show: the body of Hover for a translation, without its header, or the reason of a failure. |
| `text` | The text to copy: the translation as it came, a dictionary entry as Markdown, or the reason of a failure. |
| `actions` | The buttons: `id` (`retry` or `delete`), `title`, and the `command` and `arguments` that the client sends back with `workspace/executeCommand`. |
| `targetLanguage` | The `target_language` setting as written, such as `ja` or `Japanese`. The extension names the language in the line above the text. |

`translation-lsp/choose` carries these fields, and the client answers with the `id` of the choice picked, as `{"id": "1"}`, or `null` when it closed without one:

| Field | Value |
| --- | --- |
| `kind` | `menu` for a menu that the user opened, which the extension shows as a Quick Pick; `confirm` for agreement asked before an operation of the user goes on, with that one choice, which the extension shows as a modal warning. |
| `message` | One line: the title of a menu, or the question. |
| `choices` | The choices, each with an `id` that names it in this request and its `title`. The server reads only the `id`, so two choices may have the same title. |

The server reads only the `id` of an answer, so other fields, such as the `title` of the choice, change nothing. It takes an `id` that names no choice, an answer without one, and `null` as no choice.

The extension answers `null` without showing anything when the request was canceled before it opened. It closes a Quick Pick and answers `null` when the server withdraws the request or the connection ends, whether the extension stops or restarts the server or the connection breaks. VS Code cannot close a modal dialog from an extension, so a dialog stays until the user closes it. Its answer then does nothing; if it agrees, the extension says once that nothing was done.

## CI and release checks

The [CI workflow](../../.github/workflows/ci.yml) has three jobs:

- **Verify** runs formatting, workspace tests, Clippy, the native release build, and the WASM release build on Windows, macOS, and Linux. It runs the tests one at a time (`RUST_TEST_THREADS` is `1`), so the tests that measure time do not share the runner's CPUs with other tests.
- **Distribution contracts and registry compiler** runs on Linux. It builds the WASM extension with the compiler of the Zed registry, currently Rust 1.90, and runs the `xtask` checks described below.
- **VS Code extension** runs on Linux. It checks the types of the VS Code extension, runs its linter and its formatting check, runs its tests, and writes its `.vsix`.

When a dependency of the extension needs a newer compiler than the registry's, decide again how to stay compatible with the registry.

The [release assets workflow](../../.github/workflows/release-assets.yml) runs the packaging commands for every release target. When started with `draft-release` enabled, it creates a draft release that holds the six archives and `SHA256SUMS`. The extension downloads the server from the GitHub release of its own version, so publish that release before registering the version with Zed.

Local success on one OS does not establish success on the others. Before releasing, check the CI results for all three OSes and test the Zed flow on each: install the extension, locate the native server, connect, translate a comment, and open and save a document translation.

### xtask

The development-only `xtask` crate packages the native server and checks what a release is made from. CI runs these two commands:

```sh
cargo run -p xtask --locked -- check-source
cargo run -p xtask --locked -- check-notices
```

`check-source` checks that the versions, licenses, and release targets agree. `check-notices` generates the dependency notices described under [Dependency notices](#dependency-notices). `cargo run -p xtask --locked -- --help` lists the packaging commands.

### What the tests touch

Tests do not call the real APIs or read the credential directory of the user. HTTP/OIDC fixtures use synthetic credentials and do not require a ChatGPT account or an API key.

- The tests that start the language server as a process start only the mock provider or no provider. They give it a home directory of their own, which on macOS and Linux holds the folder of [`disk_cache`](../configuration.md#disk-cache).
- Windows takes the `disk_cache` folder from the OS and not from the environment, so these tests would use the cache folder of the user who runs them there. Those that turn `disk_cache` on run on macOS and Linux only. None of these tests confirms `Delete all cached translations`; they decline it. The unit tests of the server that agree to it give the server a directory of their own, or none.
- The tests of the providers that read credentials use directories of their own. The tests of stored translations pass the server a directory of their own.

The same executable run with `anthropic-key`, `deepl-key`, or `api-key` and the address of a service asks for an API key in a terminal and saves it instead of serving an editor. The tests that run it that way give it a home directory of their own and a pseudo-terminal. They run on macOS and Linux only, because Windows takes the credential directory from the OS and not from the environment.

The workspace lint `unsafe_code` denies `unsafe` code in every crate. The helper code of the pseudo-terminal tests is the only exception, allowed at the functions that need it with the reason for each.

## Provider limits

The chat template of PLaMo 3 Translate passes a text that starts with `<|plamo:tag|>dataset<|plamo:msg|>translation` to the model unchanged and ignores the system message. Such a text is translated without the settings that `plamo_translate` sends. Only a document about the model's prompt format would start so, and the server leaves it as it is.

## Stored translations

A stored translation is found by a key: a SHA-256 of these values.

- `FORMAT_VERSION` in `src/store/key.rs`
- what the provider's `TranslationProvider::identity` gives for the output
- `target_language` and `source_language`
- the kind of result
- the text sent

Raise `FORMAT_VERSION` when a change to what is sent changes the translations that come back, such as a change to the instructions of `providers/prompt.rs` or `providers/dictionary.rs`, the segments of `providers/segments.rs`, or the system message of `providers/plamo_translate.rs`. The translations stored before the change are then no longer found, and `disk_cache` removes them when they go unused.

A new provider implements `identity` with its name in the settings and every setting it sends that changes a translation. It leaves out what changes only how fast the reply comes, such as `speed`, and what a translation is made with, such as the key.

A file on disk also holds the version of its own layout, `FILE_VERSION` in `src/store/disk.rs`; a file of another version is not read.

## Size limit ceilings

The ceilings of `translation_limits`, listed under [Size limits](../configuration.md#size-limits), bound the memory that configured worst cases can take.

- The input budget bounds accepted source payloads. The separate input limits allow large documents while keeping Hover and selection input small.
- The output budget is smaller, because receiving a translation also holds escaped SSE events and several decoded snapshots of the output.
- The job and cache-entry limits also bound the scans of the scheduler and the cache when texts are tiny.
- Receive limits account for JSON escaping, repeated output snapshots, and SSE event overhead, and scale with `output_kib`. They bound transport processing apart from the decoded translation and do not allocate the maximum up front.

## Dependencies

Specify dependencies with Cargo compatibility ranges written as `major.minor`. Document a compatibility reason for any exact version pin. Keep related Oxc crates on the same release line.

When updating dependencies, review the lockfile changes from `cargo update`, run the checks above, and build both the native server and WASM extension in release mode. When updating `zed_extension_api`, also check the target Zed host's API support.

The Rust lexer does not compile unless `unicode-ident` and `unicode-properties` use the same Unicode version. When an update moves only one of them to a newer Unicode version, pin that one to its previous version, as the one exception to compatibility ranges, and remove the pin when the other follows.

## Dependency notices

Each release archive of the server contains `THIRD_PARTY_LICENSES.txt`: the licenses of the server's dependencies on that target, and where to download the source of each. `xtask` generates the file while packaging and does not keep it in the repository. It runs [cargo-about](https://github.com/EmbarkStudios/cargo-about) 0.9.2 with `about.toml` and the template `xtask/notices.hbs`. Install that version with:

```sh
cargo install --locked --features cli cargo-about@0.9.2
```

`about.toml` lists the accepted licenses. A dependency that offers none of them makes `check-notices` and packaging fail; review its license before adding it to the list.

`THIRD_PARTY_LICENSES.txt` gives each license in the text of the crate's authors, with their copyright notice. Where cargo-about finds no license file in a crate by itself, `about.toml` names the file and its SHA-256 checksum.

For a crate that ships no license file, `about.toml` names a file of the crate's repository on GitHub. cargo-about downloads that file, at the commit the crate was published from, every time it generates the notices. Generating them therefore needs network access.

When a checksum differs or a download fails, cargo-about uses the standard text of the license, which names no copyright holder, and still succeeds. `xtask` then fails and names the crates; if a download failed, it first runs cargo-about again, up to three times in all. `STANDARD_TEXT` in `xtask/src/notices.rs` lists the crates that may stand under a standard text, each with the reason; `THIRD_PARTY_LICENSES.txt` says so above that text.

cargo-about uses the license that `about.toml` gives for a crate in place of the license the crate declares, and its checksums cover only the files named there. `xtask` therefore also fails when a crate declares another license than its entry gives. `DECLARED` in `xtask/src/notices.rs` lists the crates whose entry differs from the declaration on purpose, each with the declaration that the entry was written for.

When `check-notices` fails after a dependency update, read the license file of each crate it names before changing `about.toml`. Then:

- If the file changed, review the change, then set the checksum to the SHA-256 checksum of the new file.
- If `about.toml` does not name the crate, add an entry for its license file.
- If a crate declares another license, also look for license files that the crate added. Then update its entry, and `DECLARED` if the entry differs from the declaration on purpose.

A file of a crate is in Cargo's registry source directory. A file of a repository is at `https://raw.githubusercontent.com/<owner>/<repository>/<commit>/<path>`, with the commit that the crate's `.cargo_vcs_info.json` records. `ra-ap-rustc_lexer` records no commit there, so also copy the commit named in its description to `override-git-commit`.

To change the cargo-about version, change it in `xtask/src/notices.rs` and in both workflows.
