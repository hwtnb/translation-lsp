# 対応言語

[English](../languages.md)

このページでは、コメントを翻訳できる言語、コメントが翻訳されないときの対処、言語ごとの言語サーバーの設定を説明します。

## 対応する言語

ホバーは、次の言語のコメントを翻訳します。

| 言語 | 翻訳するもの |
| --- | --- |
| Rust | `//` と `/* ... */`、ドキュメントコメントの `///`、`//!`、`/** ... */`、`/*! ... */` |
| JavaScript、JSX、TypeScript、TSX | `//` と `/* ... */`。JSDocのコメントと、`{/* ... */}` のようなJSXの中のコメントも含みます |
| C# | `//`、`/* ... */`、`///`、`/** ... */`。XMLドキュメントはタグ付きの文字列として表示します |
| C、C++ | `//` と `/* ... */`、`///`、`//!`、`/**`、`/*!`、`///<` などのDoxygenのコメント。`@param` などのコマンドは本文に残ります |
| Python | `#` のコメントと、モジュール・クラス・関数の先頭のドキュメンテーション文字列 |
| Go | `//` と `/* ... */`。`//go:build` などのディレクティブは除外します |
| Java | `//` と `/* ... */`、ドキュメントコメントの `/** ... */` と `///` |
| Kotlin | `//` と `/* ... */`。KDocのコメントも含みます |
| Swift | `//` と `/* ... */`、ドキュメントコメントの `///` と `/** ... */` |
| Ruby | `#` のコメントと、`=begin` ... `=end` のブロック |
| PHP | PHPのコードの中の `#`、`//`、`/* ... */`、`/** ... */`。コードの外のHTMLのコメントは翻訳しません |
| HTML | `<!-- ... -->` と、`<script>` の中のJavaScriptのコメント、`<style>` の中のCSSのコメント |
| CSS | `/* ... */` |
| SCSS | `//` と `/* ... */`。SassDocのコメントの `///` も含みます |
| Vue.js、Svelte、Astro | マークアップの `<!-- ... -->` と、JavaScriptとTypeScriptのスクリプト、CSS、SCSS、Lessのスタイルシートのコメント。SvelteとAstroでは、マークアップの `{ ... }` の式の中のコメントも翻訳します |
| XML | `<!-- ... -->` |
| SQL | `--` と `/* ... */`。MySQLの `#` のコメントは見つけません |
| Lua | `--` のコメントと、`--[[ ... ]]` のような長いコメント。LuaCATSのコメントの `---` も含みます |
| Zig | `//` と、ドキュメントコメントの `///` と `//!` |
| Protocol Buffers | `//` と `/* ... */` |
| GraphQL | `#` のコメント。説明（description）は文字列なので翻訳しません |
| Clojure | `;` のコメント。ドキュメンテーション文字列は翻訳しません |
| Haskell | `--` と `{- ... -}`。Haddockのコメントも含みます |
| OCaml、OCamlのインターフェース | `(* ... *)` と、ドキュメントコメントの `(** ... *)` |
| Scala | `//` と `/* ... */`。Scaladocのコメントも含みます |
| Dart | `//` と `/* ... */`、ドキュメントコメントの `///` と `/** ... */` |
| Elixir | `#` のコメントと、`@moduledoc`、`@doc`、`@typedoc` に渡した文字列 |
| Terraform、Terraform Vars、HCL | `#` と `//` のコメント、`/* ... */`（文字列とヒアドキュメントの本文を除く） |
| Nix | `#` のコメント、`/* ... */`、ドキュメントコメントの `/** ... */`（文字列の本文を除く） |
| Prisma | `//` のコメント、`/* ... */`、ドキュメントコメントの `///` |
| Gleam | `//` のコメント、ドキュメントコメントの `///` と `////` |
| Erlang | `%` のコメントと、`-moduledoc` と `-doc` に渡した文字列 |
| CMake | `#` のコメントと `#[[ ... ]]` 形式のコメント |
| GLSL | `//` と `/* ... */` |
| JSON、JSONC | `//` と `/* ... */` |
| YAML、TOML | `#` のコメント |
| シェルスクリプト | sh、Bash、zshのスクリプトの `#` のコメント |
| PowerShell | `#` と `<# ... #>`。`#Requires` の行は除外します |
| Dockerfile | ヒアドキュメントの外で、字下げの後が `#` で始まる行。`# syntax=...` などのパーサー指定は除外します |
| Docker Compose | `#` のコメント。YAMLとして読みます |
| メイクファイル | `#` のコメント。レシピの行のコメントも含みます |

MarkdownとPlain Textには、読み取るコメントがありません。文章を選択してコードアクションで翻訳するか、文書全体を翻訳して別のタブに開いてください。Markdownでは、本文を選択して安定させるだけでも翻訳を始めます。[Markdownの選択](usage.md#markdownの選択)で説明します。

## コメントが翻訳されないとき

サーバーは、文書を実行せず、その言語と同じように読みます。文字列などのリテラルの中は、コメントとして扱いません。キーや文字列の値を訳すときは、範囲を選択して `Translate selection to …` を使います。

連続する行の行コメントは、種類と字下げが同じで、どれもコードと同じ行にないとき、ふつうはまとめて翻訳します。ブロックコメント、ドキュメンテーション文字列、HTMLのコメントは、1つずつ翻訳します。

書きかけの箇所や、サーバーがその言語と同じように読めない箇所では、言語によって次のどれかになります。

- 閉じていないブロックコメントは、閉じるまで翻訳せず、それより後のコメントも見つけません。
- 閉じていない文字列などのリテラルは、その言語の読み方に従って、それより後のコメントを隠すか、その行の末尾で終わります。
- 構文エラーの周りや、その言語が複数の読み方をしうる箇所では、コメントを対象から外すか、そこで抽出を止めます。そこより前のコメントは抽出できます。
- まれな書き方では、その言語がコメントとして読まない文字列をコメントとして扱うことがあります。

どの場合も、範囲を選択して `Translate selection to …` を使えば翻訳できます。

JavaScriptとTypeScriptでは、約10万段を超える入れ子（`[` を10万個ほど続けたものなど）があると、言語サーバーが停止します。HTML文書とコンポーネントファイルのスクリプトも同じです。

言語ごとの規則は[コメントの読み取り](development/comment-readers.md)で説明します。

### コメントを読み取れなかった文書

サーバーは、ホバー、CodeLens、翻訳で最初に必要になったときに文書のコメントを読み取り、編集のあとにもう一度読み取ります。C、C++、C#、Pythonでだけ、読み取りが `comment_parse_timeout_ms`（既定値は1秒）を超えると、そこで止めます。

読み取りが上限で止まった場合と、サーバーの中で失敗した場合、サーバーはファイル名を示す警告を1回表示します。それ以降、その文書ではコメントを認識しません。編集しても、読み取りをやり直しません。文書を閉じて開き直すと、コメントをもう一度読み取ります。ほかの文書には影響しません。

範囲選択による翻訳は、その文書でも使えます。コメントをちょうど選択した場合も、コメントの記号を含めて、ほかの選択と同じように扱います。短い選択の扱いは[単語の意味を調べる](usage.md#単語の意味を調べる)で説明します。

## 言語サーバーの設定

設定で言語の言語サーバーを明示している場合は、既存のサーバーに `"translation-lsp"` を追加します。その一覧は `languages.<設定上の言語名>.language_servers` です。

| 言語 | 設定上の言語名 | Zedでの用意 |
| --- | --- | --- |
| Rust | `Rust` | 組み込み |
| JavaScript | `JavaScript` | 組み込み |
| TypeScript | `TypeScript` | 組み込み |
| TSX | `TSX` | 組み込み |
| C# | `CSharp` | [C#拡張](https://github.com/zed-extensions/csharp) |
| C | `C` | 組み込み |
| C++ | `C++` | 組み込み |
| Python | `Python` | 組み込み |
| Go | `Go` | 組み込み |
| Java | `Java` | [Java拡張](https://github.com/zed-extensions/java) |
| Kotlin | `Kotlin` | [Kotlin拡張](https://github.com/zed-extensions/kotlin) |
| Swift | `Swift` | [Swift拡張](https://github.com/zed-extensions/swift) |
| Ruby | `Ruby` | [Ruby拡張](https://github.com/zed-extensions/ruby) |
| PHP | `PHP` | [PHP拡張](https://github.com/zed-extensions/php) |
| HTML | `HTML` | Zedが既定で導入するHTML拡張 |
| Vue.js | `Vue.js` | [Vue拡張](https://github.com/zed-extensions/vue) |
| Svelte | `Svelte` | [Svelte拡張](https://github.com/zed-extensions/svelte) |
| Astro | `Astro` | [Astro拡張](https://github.com/zed-extensions/astro) |
| XML | `XML` | [XML拡張](https://github.com/sweetppro/zed-xml) |
| CSS | `CSS` | 組み込み |
| SCSS | `SCSS` | [SCSS拡張](https://github.com/bajrangCoder/zed-scss) |
| SQL | `SQL` | [SQL拡張](https://github.com/zed-extensions/sql) |
| Lua | `Lua` | [Lua拡張](https://github.com/zed-extensions/lua) |
| Zig | `Zig` | [Zig拡張](https://github.com/zed-extensions/zig) |
| Protocol Buffers | `Proto` | [Proto拡張](https://github.com/zed-industries/zed/tree/main/extensions/proto) |
| GraphQL | `GraphQL` | [GraphQL拡張](https://github.com/11bit/zed-extension-graphql) |
| Clojure | `Clojure` | [Clojure拡張](https://github.com/zed-extensions/clojure) |
| Haskell | `Haskell` | [Haskell拡張](https://github.com/zed-extensions/haskell) |
| OCaml | `OCaml` | [OCaml拡張](https://github.com/zed-extensions/ocaml) |
| OCamlのインターフェース | `OCaml Interface` | [OCaml拡張](https://github.com/zed-extensions/ocaml) |
| Scala | `Scala` | [Scala拡張](https://github.com/scalameta/metals-zed) |
| Dart | `Dart` | [Dart拡張](https://github.com/zed-extensions/dart) |
| Elixir | `Elixir` | [Elixir拡張](https://github.com/zed-extensions/elixir) |
| Terraform | `Terraform` | [Terraform拡張](https://github.com/zed-extensions/terraform) |
| Terraform Vars | `Terraform Vars` | [Terraform拡張](https://github.com/zed-extensions/terraform) |
| HCL | `HCL` | [Terraform拡張](https://github.com/zed-extensions/terraform) |
| Nix | `Nix` | [Nix拡張](https://github.com/zed-extensions/nix) |
| Prisma | `Prisma` | [Prisma拡張](https://github.com/zed-extensions/prisma) |
| Gleam | `Gleam` | [Gleam拡張](https://github.com/gleam-lang/zed-gleam) |
| Erlang | `Erlang` | [Erlang拡張](https://github.com/zed-extensions/erlang) |
| CMake | `CMake` | [neocmake拡張](https://github.com/k0tran/zed_neocmake) |
| GLSL | `GLSL` | [GLSL拡張](https://github.com/zed-industries/zed/tree/main/extensions/glsl) |
| JSON | `JSON` | 組み込み |
| JSONC | `JSONC` | 組み込み |
| YAML | `YAML` | 組み込み |
| TOML | `TOML` | [TOML拡張](https://github.com/zed-extensions/toml) |
| シェルスクリプト | `Shell Script` | 組み込み |
| PowerShell | `PowerShell` | [PowerShell拡張](https://github.com/zed-extensions/powershell) |
| Dockerfile | `Dockerfile` | [Dockerfile拡張](https://github.com/zed-extensions/dockerfile) |
| Docker Compose | `Docker Compose` | [Dockerfile拡張](https://github.com/zed-extensions/dockerfile) |
| メイクファイル | `Make` | [Make拡張](https://github.com/caius/zed-make) |
| Markdown | `Markdown` | 組み込み |
| Plain Text | `Plain Text` | 組み込み |

- C#では、この拡張はRoslynやOmniSharpを置き換えません。並べて追加してください。
- OCaml拡張は、OCamlとOCamlのインターフェースの両方を提供します。Dockerfile拡張は、DockerfileとDocker Composeの両方を提供します。
- Zedは、`.jsx` に `JavaScript`、`.tsx` に `TSX` の設定を使います。
- `Shell Script` は、`.sh`、`.bash`、`.zsh`、`.bashrc`、`.zshrc` などのファイルを扱います。

どの言語にも共通する規則は[言語サーバーの一覧](configuration.md#言語サーバーの一覧)で説明します。
