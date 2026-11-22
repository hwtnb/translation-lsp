# Languages

[日本語](ja/languages.md)

This page lists the languages whose comments are translated, what to do when a comment is not translated, and the language server settings of each language.

## Supported languages

Hover translates the comments of the following languages.

| Language | What is translated |
| --- | --- |
| Rust | `//` and `/* ... */`, and the documentation comments `///`, `//!`, `/** ... */`, and `/*! ... */` |
| JavaScript, JSX, TypeScript, TSX | `//` and `/* ... */`, including JSDoc comments and comments in JSX such as `{/* ... */}` |
| C# | `//`, `/* ... */`, `///`, and `/** ... */`. XML documentation is shown as tagged text |
| C, C++ | `//` and `/* ... */`, and Doxygen comments such as `///`, `//!`, `/**`, `/*!`, and `///<`. Commands such as `@param` stay in the text |
| Python | `#` comments, and docstrings at the beginning of a module, class, or function body |
| Go | `//` and `/* ... */`. Directives such as `//go:build` are skipped |
| Java | `//` and `/* ... */`, and the documentation comments `/** ... */` and `///` |
| Kotlin | `//` and `/* ... */`, including KDoc comments |
| Swift | `//` and `/* ... */`, and the documentation comments `///` and `/** ... */` |
| Ruby | `#` comments and `=begin` ... `=end` blocks |
| PHP | `#`, `//`, `/* ... */`, and `/** ... */` in PHP code. HTML comments outside the code are not translated |
| HTML | `<!-- ... -->`, and the comments of JavaScript in `<script>` and of CSS in `<style>` |
| CSS | `/* ... */` |
| SCSS | `//` and `/* ... */`, including SassDoc comments `///` |
| Vue.js, Svelte, Astro | `<!-- ... -->` in the markup, and the comments of JavaScript and TypeScript scripts and of CSS, SCSS, and Less style sheets. In Svelte and Astro, comments in the `{ ... }` expressions of the markup too |
| XML | `<!-- ... -->` |
| SQL | `--` and `/* ... */`. A MySQL `#` comment is not found |
| Lua | `--` comments and long comments such as `--[[ ... ]]`, including LuaCATS comments `---` |
| Zig | `//`, and the documentation comments `///` and `//!` |
| Protocol Buffers | `//` and `/* ... */` |
| GraphQL | `#` comments. A description is a string and is not translated |
| Clojure | `;` comments. A docstring is not translated |
| Haskell | `--` and `{- ... -}`, including Haddock comments |
| OCaml, OCaml interfaces | `(* ... *)`, and the documentation comments `(** ... *)` |
| Scala | `//` and `/* ... */`, including Scaladoc comments |
| Dart | `//` and `/* ... */`, and the documentation comments `///` and `/** ... */` |
| Elixir | `#` comments, and the strings given to `@moduledoc`, `@doc`, and `@typedoc` |
| Terraform, Terraform Vars, HCL | `#` and `//` comments, and `/* ... */`, outside the literal text of strings and heredocs |
| Nix | `#` comments, `/* ... */`, and `/** ... */` documentation comments, outside the literal text of strings |
| Prisma | `//` and `/* ... */` comments, including `///` documentation comments |
| Gleam | `//` comments, including `///` and `////` documentation comments |
| Erlang | `%` comments, and the strings given to `-moduledoc` and `-doc` |
| CMake | `#` comments and bracket comments such as `#[[ ... ]]` |
| GLSL | `//` and `/* ... */` |
| JSON, JSONC | `//` and `/* ... */` |
| YAML, TOML | `#` comments |
| Shell scripts | `#` comments in sh, Bash, and zsh scripts |
| PowerShell | `#` and `<# ... #>`. `#Requires` lines are skipped |
| Dockerfile | Lines outside here-documents that start with `#` after their indentation. Parser directives such as `# syntax=...` are skipped |
| Docker Compose | `#` comments, read as YAML |
| Makefiles | `#` comments, also in recipe lines |

Markdown and Plain Text have no comments to read. Select text and use the Code Actions to translate it, or translate the whole document in a tab. In Markdown, a stable selection of prose also starts a translation by itself, as described under [Markdown selection](usage.md#markdown-selection).

## When a comment is not translated

The server reads a document as its language reads it, without running it. Text inside strings and other literals is not a comment. To translate a key or a string value, select it and use `Translate selection to …`.

Line comments on consecutive lines are usually translated together when they are of the same kind, have the same indentation, and stand alone on their lines. A block comment, a docstring, and an HTML comment are each translated on their own.

Where the text is unfinished, or where the server cannot read it as the language does, the result is one of the following, depending on the language:

- An unclosed block comment is not translated, and no comment after it is found, until it is closed.
- An unclosed string or other literal hides the comments after it, or ends at the end of its line, as the language reads it.
- Around a syntax error, or where the language can read the text in more than one way, a comment is left out, or extraction stops there. The comments before that point remain available.
- In some rare forms, text that the language does not read as a comment can be taken as one.

In all of these cases, select the text and use `Translate selection to …` to translate it.

In JavaScript and TypeScript, code nested more than about 100,000 levels deep, such as that many `[` in a row, stops the language server. This also holds for the scripts of an HTML document or a component file.

The rules of each language are described under [Comment readers](development/comment-readers.md).

### Documents whose comments cannot be read

The server reads the comments of a document when a Hover, a CodeLens, or a translation first needs them, and again after an edit. For C, C++, C#, and Python only, the reading stops once it has taken longer than `comment_parse_timeout_ms`, which is 1 second by default.

When the reading stops at the limit, or fails inside the server, the server shows one warning that names the file. From then on no comment is recognized in that document, and editing it does not start another reading. Close the document and open it again to have its comments read once more. Other documents are not affected.

Selection translation keeps working in that document. A selection that is exactly a comment is handled as any other selection, with its comment marks. How a short selection is handled is described under [Looking up a word](usage.md#looking-up-a-word).

## Language server settings

If your settings list the language servers of a language explicitly, add `"translation-lsp"` alongside your existing servers. The list is `languages.<settings key>.language_servers`.

| Language | Settings key | Provided by |
| --- | --- | --- |
| Rust | `Rust` | Built in |
| JavaScript | `JavaScript` | Built in |
| TypeScript | `TypeScript` | Built in |
| TSX | `TSX` | Built in |
| C# | `CSharp` | [C# extension](https://github.com/zed-extensions/csharp) |
| C | `C` | Built in |
| C++ | `C++` | Built in |
| Python | `Python` | Built in |
| Go | `Go` | Built in |
| Java | `Java` | [Java extension](https://github.com/zed-extensions/java) |
| Kotlin | `Kotlin` | [Kotlin extension](https://github.com/zed-extensions/kotlin) |
| Swift | `Swift` | [Swift extension](https://github.com/zed-extensions/swift) |
| Ruby | `Ruby` | [Ruby extension](https://github.com/zed-extensions/ruby) |
| PHP | `PHP` | [PHP extension](https://github.com/zed-extensions/php) |
| HTML | `HTML` | HTML extension, which Zed installs by default |
| Vue.js | `Vue.js` | [Vue extension](https://github.com/zed-extensions/vue) |
| Svelte | `Svelte` | [Svelte extension](https://github.com/zed-extensions/svelte) |
| Astro | `Astro` | [Astro extension](https://github.com/zed-extensions/astro) |
| XML | `XML` | [XML extension](https://github.com/sweetppro/zed-xml) |
| CSS | `CSS` | Built in |
| SCSS | `SCSS` | [SCSS extension](https://github.com/bajrangCoder/zed-scss) |
| SQL | `SQL` | [SQL extension](https://github.com/zed-extensions/sql) |
| Lua | `Lua` | [Lua extension](https://github.com/zed-extensions/lua) |
| Zig | `Zig` | [Zig extension](https://github.com/zed-extensions/zig) |
| Protocol Buffers | `Proto` | [Proto extension](https://github.com/zed-industries/zed/tree/main/extensions/proto) |
| GraphQL | `GraphQL` | [GraphQL extension](https://github.com/11bit/zed-extension-graphql) |
| Clojure | `Clojure` | [Clojure extension](https://github.com/zed-extensions/clojure) |
| Haskell | `Haskell` | [Haskell extension](https://github.com/zed-extensions/haskell) |
| OCaml | `OCaml` | [OCaml extension](https://github.com/zed-extensions/ocaml) |
| OCaml interfaces | `OCaml Interface` | [OCaml extension](https://github.com/zed-extensions/ocaml) |
| Scala | `Scala` | [Scala extension](https://github.com/scalameta/metals-zed) |
| Dart | `Dart` | [Dart extension](https://github.com/zed-extensions/dart) |
| Elixir | `Elixir` | [Elixir extension](https://github.com/zed-extensions/elixir) |
| Terraform | `Terraform` | [Terraform extension](https://github.com/zed-extensions/terraform) |
| Terraform Vars | `Terraform Vars` | [Terraform extension](https://github.com/zed-extensions/terraform) |
| HCL | `HCL` | [Terraform extension](https://github.com/zed-extensions/terraform) |
| Nix | `Nix` | [Nix extension](https://github.com/zed-extensions/nix) |
| Prisma | `Prisma` | [Prisma extension](https://github.com/zed-extensions/prisma) |
| Gleam | `Gleam` | [Gleam extension](https://github.com/gleam-lang/zed-gleam) |
| Erlang | `Erlang` | [Erlang extension](https://github.com/zed-extensions/erlang) |
| CMake | `CMake` | [neocmake extension](https://github.com/k0tran/zed_neocmake) |
| GLSL | `GLSL` | [GLSL extension](https://github.com/zed-industries/zed/tree/main/extensions/glsl) |
| JSON | `JSON` | Built in |
| JSONC | `JSONC` | Built in |
| YAML | `YAML` | Built in |
| TOML | `TOML` | [TOML extension](https://github.com/zed-extensions/toml) |
| Shell scripts | `Shell Script` | Built in |
| PowerShell | `PowerShell` | [PowerShell extension](https://github.com/zed-extensions/powershell) |
| Dockerfile | `Dockerfile` | [Dockerfile extension](https://github.com/zed-extensions/dockerfile) |
| Docker Compose | `Docker Compose` | [Dockerfile extension](https://github.com/zed-extensions/dockerfile) |
| Makefiles | `Make` | [Make extension](https://github.com/caius/zed-make) |
| Markdown | `Markdown` | Built in |
| Plain Text | `Plain Text` | Built in |

- In C#, this extension does not replace Roslyn or OmniSharp; add it alongside them.
- The OCaml extension provides both OCaml and OCaml interfaces. The Dockerfile extension provides both Dockerfile and Docker Compose.
- Zed uses the `JavaScript` settings for `.jsx` and `TSX` for `.tsx`.
- `Shell Script` covers files such as `.sh`, `.bash`, `.zsh`, `.bashrc`, and `.zshrc`.

The rule shared by every language is described under [Language server lists](configuration.md#language-server-lists).
