# 開発

[English](../../development/README.md)

このページは、拡張と言語サーバーを開発する人のためのものです。実行する検証、モックのプロバイダーとサンプル、CIとリリース、依存関係の方針などを説明します。サーバーがコメントをどう読むかは、[コメントの読み取り](comment-readers.md)で説明します。

## 検証

各クレートはRust Edition 2024です。仮想ワークスペースには依存関係リゾルバーのバージョン3を使います。

- ネイティブ言語サーバーと `xtask` にはRust 1.98以降が必要です。古い最低対応Rustの維持より依存更新を優先し、必要に応じて対象クレートの要件を引き上げます。
- WASM拡張は `rust-version` を宣言しません。Cargoは、ワークスペースのメンバーのうち最も低い `rust-version` に合う依存のバージョンを、ワークスペース全体に選びます。そのため、Zedの登録先が使う古いコンパイラーを拡張に宣言すると、ネイティブ言語サーバーの依存まで古いバージョンになります。拡張がそのコンパイラーでビルドできることは、CIで確かめます。

`Cargo.lock` をバージョン管理し、検証とリリースビルドは `--locked` で実行してください。

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## モックのプロバイダー

設定で `"provider": { "name": "mock" }` にすると、サーバーはモックのプロバイダーを使います。文章を外部へ送らず、同梱サンプルの決まった日本語訳を返すため、アカウントやAPIキーなしでZedでの流れを確かめられます。

モックのプロバイダーは、下のコメントのサンプルと「モック」と書いたサンプル、その改行の違い、語 `retry_count` だけに対応します。ほかの本文や日本語以外の訳先ではエラーを返します。ホバーの見出しには `Mock translation (fixed sample)` と表示します。

### コメントのサンプル

次のサンプルのコメントにマウスポインターを重ねてください。

| 言語 | サンプル |
| --- | --- |
| Rust | [samples/comment.rs](../../../samples/comment.rs)。コメントの `Settings` のメニューを開いたままでも、コメントにホバーすると訳文が出ます。 |
| C# | [samples/comment.cs](../../../samples/comment.cs)。通常コメントとXMLドキュメントコメント |
| C、C++ | [samples/comment.c](../../../samples/comment.c)、[samples/comment.cpp](../../../samples/comment.cpp)。Doxygenコメントも含む |
| JavaScript、JSX | [samples/comment.js](../../../samples/comment.js)、[samples/comment.jsx](../../../samples/comment.jsx) |
| TypeScript、TSX | [samples/comment.ts](../../../samples/comment.ts)、[samples/comment.tsx](../../../samples/comment.tsx) |
| Python | [samples/comment.py](../../../samples/comment.py)。コメントとドキュメンテーション文字列 |
| Go、Java、Kotlin | [samples/comment.go](../../../samples/comment.go)、[samples/comment.java](../../../samples/comment.java)、[samples/comment.kt](../../../samples/comment.kt) |
| Swift、Ruby | [samples/comment.swift](../../../samples/comment.swift)、[samples/comment.rb](../../../samples/comment.rb) |
| PHP、SQL | [samples/comment.php](../../../samples/comment.php)、[samples/comment.sql](../../../samples/comment.sql) |
| HTML | [samples/comment.html](../../../samples/comment.html)。HTMLのコメントと、スクリプトとスタイルシートの中のコメント |
| CSS、SCSS | [samples/comment.css](../../../samples/comment.css)、[samples/comment.scss](../../../samples/comment.scss) |
| Vue.js、Svelte、Astro | [samples/comment.vue](../../../samples/comment.vue)、[samples/comment.svelte](../../../samples/comment.svelte)、[samples/comment.astro](../../../samples/comment.astro) |
| XML、Lua、Zig | [samples/comment.xml](../../../samples/comment.xml)、[samples/comment.lua](../../../samples/comment.lua)、[samples/comment.zig](../../../samples/comment.zig) |
| Protocol Buffers、GraphQL、メイクファイル | [samples/comment.proto](../../../samples/comment.proto)、[samples/comment.graphql](../../../samples/comment.graphql)、[samples/Makefile](../../../samples/Makefile) |
| Clojure、Haskell | [samples/comment.clj](../../../samples/comment.clj)、[samples/comment.hs](../../../samples/comment.hs) |
| OCaml | [samples/comment.ml](../../../samples/comment.ml)、[samples/comment.mli](../../../samples/comment.mli) |
| Scala、Dart | [samples/comment.scala](../../../samples/comment.scala)、[samples/comment.dart](../../../samples/comment.dart) |
| Elixir | [samples/comment.ex](../../../samples/comment.ex)。コメントとドキュメント |
| Terraform、Terraform Vars、HCL | [samples/comment.tf](../../../samples/comment.tf)、[samples/comment.tfvars](../../../samples/comment.tfvars)、[samples/comment.hcl](../../../samples/comment.hcl) |
| Nix | [samples/comment.nix](../../../samples/comment.nix) |
| Prisma | [samples/comment.prisma](../../../samples/comment.prisma) |
| Gleam | [samples/comment.gleam](../../../samples/comment.gleam) |
| Erlang | [samples/comment.erl](../../../samples/comment.erl) |
| CMake | [samples/comment.cmake](../../../samples/comment.cmake) |
| GLSL | [samples/comment.frag](../../../samples/comment.frag) |
| F# | [samples/comment.fsx](../../../samples/comment.fsx) |
| JSON、JSONC | [samples/comment.json](../../../samples/comment.json)、[samples/comment.jsonc](../../../samples/comment.jsonc)。`comment.json` のコメントは、ZedのJSONの言語サーバーが問題として報告しますが、ホバーは同じように翻訳します。 |
| YAML、TOML | [samples/comment.yaml](../../../samples/comment.yaml)、[samples/comment.toml](../../../samples/comment.toml) |
| シェルスクリプト、PowerShell | [samples/comment.sh](../../../samples/comment.sh)、[samples/comment.ps1](../../../samples/comment.ps1) |
| Dockerfile、Docker Compose | [samples/Dockerfile](../../../samples/Dockerfile)、[samples/compose.yaml](../../../samples/compose.yaml) |

### ほかのサンプル

| サンプル | プロバイダー | 確かめること |
| --- | --- | --- |
| [samples/hover.rs](../../../samples/hover.rs)、[samples/hover.java](../../../samples/hover.java)、[samples/hover.py](../../../samples/hover.py) | モック | 訳文の見出し、リスト、表、コード、URLをホバーがどう表示するか。`hover.java` にはJavadocのコメントが、`hover.py` には節のあるドキュメンテーション文字列があります。`hover.rs` の `report` のコメントには `--`、引用符、`...` があり、CodeLensのカードでも書いたとおりに表示されます。 |
| [samples/paragraph.md](../../../samples/paragraph.md) | モック | 段落を選択し、コードアクションの `Translate selection to Japanese (ja)` を実行してから、選択範囲にマウスポインターを重ねます。単語の説明を見るには、`retry_count` だけを選択して、同じ操作を実行します。モックのプロバイダーでは、Markdownは手動実行が必要です。 |
| [samples/document.txt](../../../samples/document.txt) | モックと実際 | Plain Textのサンプルです。最初の段落は `paragraph.md` の段落と同じで、モックのプロバイダーで選択の翻訳と単語の説明を試せます。その後の2つの段落は、実際のプロバイダーでタブでの翻訳を試すためのものです。 |
| [samples/document.md](../../../samples/document.md) | 実際 | タブで開いた翻訳がMarkdownを保つことと、コードだけやリンク先だけを選択しても自動の翻訳が始まらないこと。サンプルには、見出し、リスト、表、コードブロック、行の中のコード、強調と、3種類のリンクがあります。`https` のURL、文書の中の見出し、同じフォルダーのファイルへのリンクです。 |
| [samples/languages.rs](../../../samples/languages.rs) | 実際 | 翻訳先の言語だとしてホバーが訳さないのがどのコメントか。[翻訳先の言語ですでに書かれた文章](#翻訳先の言語ですでに書かれた文章)を参照してください。 |
| [samples/languages.md](../../../samples/languages.md) | 実際 | `features.automatic_selection` を `true`、`target_language` を `ja` にして段落を選択すると、日本語の2つの段落にはホバーが出ず、英語の段落には出ること。 |
| [samples/long-document.md](../../../samples/long-document.md) | モック | 部分に分けて送る翻訳。[部分に分けて送る翻訳](#部分に分けて送る翻訳)を参照してください。 |
| [samples/repeated.rs](../../../samples/repeated.rs) | モックと実際 | 依頼を送らずに表示する訳文。[繰り返しの翻訳](#繰り返しの翻訳)を参照してください。 |

モックのプロバイダーは、「実際」とだけ書いたサンプルを訳しません。

### 翻訳先の言語ですでに書かれた文章

[samples/languages.rs](../../../samples/languages.rs) には、次のコメントがあります。

- 英語、日本語、コードとURLを含む日本語
- 英語と日本語が半分ずつのもの
- 漢字だけのもの、中国語（簡体字）
- 日本語の中に英語が1文だけある6文
- キリル文字のセルビア語。`target_language` を `sr-Latn` にすると、ホバーはセルビア語だとして隠さず、ラテン文字の訳文を表示します。

### 部分に分けて送る翻訳

`document_split` を `{"enabled": true, "chunk_kib": 1}` にします。すると、`Translate document to Japanese (ja) and open in tab` は [samples/long-document.md](../../../samples/long-document.md) を4つの部分に分けて送ります。そのうち1つは節 `Closing connections` の途中から始まり、この節のコードブロックには `#` で始まる行があります。

モックのプロバイダーは、このサンプルを部分に分けて送ったときだけ訳します。`document_split` がないときや、`chunk_kib` が大きくて分かれないときは、1つのまま送ります。そのときは、同梱のサンプルしか扱えないという誤りを返します。

### 繰り返しの翻訳

[samples/repeated.rs](../../../samples/repeated.rs) には、`comment.rs` のコメントを2つと、実際のプロバイダー用のドキュメントコメントを2つ入れています。片方にホバーしてからもう片方にホバーすると、2つ目は依頼を送らずに表示します。[キャッシュ](../behavior.md#キャッシュ)で説明するとおりです。[`disk_cache`](../configuration.md#ディスクキャッシュ) を有効にすると、Zedを起動し直したあとも依頼を送らずに表示します。

モックのプロバイダーで `disk_cache` を有効にすると、そのフォルダーのファイルの数を前後で数えて、タブが新しく送った部分の数を確かめられます。例えば、次のようにします。

1. `document_split` を `long-document.md` のときと同じにして、そのサンプルをタブに翻訳します。
2. リストの項目を1つ消してから、もう一度翻訳します。
3. 変わった部分の1つ分だけファイルが増えます。モックのプロバイダーは、短くなった部分も訳します。

ほかの場合には、ファイルの数は依頼の数になりません。やり直しはファイルを置き換え、DeepL APIやGoogle Cloud Translationは1つの翻訳に複数の依頼を送ることがあります。

### stdioのテストの引数

stdioのテストは、実行ファイルを `--mock-delay-ms <ミリ秒>`、`--mock-fail`、`--mock-timeout-ms <ミリ秒>`、`--mock-same-language` 付きで起動します。それぞれ、モックの翻訳を遅らせる、失敗させる、期限を指定の値にする、自動の翻訳で文章をすでに翻訳先の言語だとする、の働きをします。これらの引数はモックのプロバイダーにだけ効きます。

## CIとリリース前の確認

[CIワークフロー](../../../.github/workflows/ci.yml)には2つのジョブがあります。

- **Verify** は、Windows、macOS、Linuxそれぞれで動きます。fmt、ワークスペース全体のテスト、Clippy、ネイティブとWASMのリリースビルドを実行します。テストは1つずつ実行します（`RUST_TEST_THREADS` が `1`）。時間を測るテストが、ほかのテストとランナーのCPUを取り合わないようにするためです。
- **Distribution contracts and registry compiler** は、Linuxで動きます。Zedの登録先のコンパイラー（現在はRust 1.90）でWASM拡張をビルドし、下で説明する `xtask` の検査を実行します。

拡張の依存が登録先より新しいコンパイラーを必要とするようになったら、登録先との互換性をどう保つかを改めて決めてください。

[配布物を用意するワークフロー](../../../.github/workflows/release-assets.yml)は、配布対象ごとに梱包用のコマンドを実行します。`draft-release` を有効にして起動すると、6本のアーカイブと `SHA256SUMS` を付けた下書きのリリースを作ります。拡張は、自分と同じバージョンのGitHubリリースからサーバーを取得します。そのため、そのリリースを公開してから、そのバージョンをZedに登録してください。

1 OSでの成功だけを他OSの成功とは扱いません。リリース前には、3 OSのCI結果を確かめてください。さらに各OSのZedで、拡張を入れる、ネイティブの言語サーバーを見つける、接続する、コメントを翻訳する、文書の翻訳を開いて保存する、を確かめてください。

### xtask

開発専用の `xtask` クレートは、ネイティブ言語サーバーを梱包し、リリースの元になる情報を検査します。CIは次の2つのコマンドを実行します。

```sh
cargo run -p xtask --locked -- check-source
cargo run -p xtask --locked -- check-notices
```

`check-source` は、バージョン、ライセンス、配布対象が互いに合っていることを検査します。`check-notices` は、[依存ライブラリのライセンス表示](#依存ライブラリのライセンス表示)で説明する文書を生成します。梱包用のコマンドは `cargo run -p xtask --locked -- --help` で一覧できます。

### テストが触れるもの

テストは実際のAPIを呼ばず、利用者の認証情報のディレクトリも読みません。HTTP/OIDCのテスト用環境は合成の認証情報を使うため、ChatGPTアカウントもAPIキーも不要です。

- 言語サーバーをプロセスとして起動するテストは、モックのプロバイダーだけを起動するか、プロバイダーなしで起動します。サーバーにはテスト用のホームディレクトリを与えます。macOSとLinuxでは、[`disk_cache`](../configuration.md#ディスクキャッシュ) のフォルダーはその中になります。
- Windowsは `disk_cache` のフォルダーを環境変数ではなくOSから得ます。そこでは、これらのテストは実行する利用者のキャッシュのフォルダーを使うことになります。このため、`disk_cache` を有効にするテストはmacOSとLinuxでだけ動きます。どのテストも `Delete all cached translations` を確定しません。
- 認証情報を読むプロバイダーのテストは、テスト用のディレクトリを使います。残した訳文のテストは、サーバーにテスト用のディレクトリを渡します。

同じ実行ファイルを `anthropic-key` か `deepl-key`、またはサービスのアドレスを添えた `api-key` 付きで実行すると、エディターとは通信しません。端末でAPIキーを尋ねて保存します。この形で実行するテストは、テスト用のホームディレクトリと擬似端末を与え、macOSとLinuxでだけ動きます。Windowsは、認証情報のディレクトリを環境変数ではなくOSから得るためです。

`unsafe` なコードは、ワークスペースのlint `unsafe_code` がすべてのクレートで拒否します。例外は擬似端末のテストの補助コードだけで、必要な関数ごとに、理由を付けて許可しています。

## 残した訳文

残した訳文は鍵で引きます。鍵は、次の値から作るSHA-256です。

- `src/store/key.rs` の `FORMAT_VERSION`
- プロバイダーの `TranslationProvider::identity` がその表示先について返すもの
- `target_language` と `source_language`
- 結果の種類
- 送った文章

送る内容を、返る訳文が変わる形で直したときは、`FORMAT_VERSION` を上げます。`providers/prompt.rs` や `providers/dictionary.rs` の指示文、`providers/segments.rs` の区切り方を直す場合です。直す前に残した訳文は見つからなくなり、`disk_cache` は使われないまま期限を過ぎたものを消します。

新しいプロバイダーは `identity` を実装し、設定でのプロバイダーの名前と、送る設定のうち訳文を左右するものをすべて入れます。`speed` のように応答の速さだけを変えるものと、キーのように翻訳に使うものは入れません。

ディスクのファイルは、自分の形式の版 `src/store/disk.rs` の `FILE_VERSION` も持ち、版の違うファイルは読みません。

## サイズの上限の基準

[サイズの上限](../configuration.md#サイズの上限)にある `translation_limits` の基準は、設定上の最大負荷が使うメモリを抑えるためのものです。

- 入力の合計容量で受付済みの原文を制限します。入力の個別上限は、大きな文書を許容しつつ、ホバーと選択範囲の入力を小さく保ちます。
- 出力の上限は入力より小さくしています。訳文を受け取る間は、エスケープされたSSEイベントと、訳文の複数のスナップショットも保持するためです。
- 件数とキャッシュ項目数の上限は、小さな文章が大量にある場合のスケジューラとキャッシュの走査も制限します。
- 通信側の制限は、JSONのエスケープ・訳文の重複送信・SSEイベントの付加情報を考慮して `output_kib` に連動します。訳文のサイズとは別に通信処理量を制限し、最大量のメモリを先に確保することはありません。

## 依存関係

依存関係のバージョンは `major.minor` のCargo互換範囲で指定します。特定のバージョンに固定する場合は、互換性上の理由を記録してください。Oxcの関連クレートは同じリリース系列に揃えます。

更新時は `cargo update` によるロックファイルの差分を確認し、上記の検証とネイティブ・WASM両方のリリースビルドを実行してください。`zed_extension_api` の更新時は、対象ZedのAPI対応も確認してください。

Rustの字句解析器は、`unicode-ident` と `unicode-properties` のUnicodeの版が同じでないとコンパイルできません。更新でどちらか一方だけが新しいUnicodeの版に上がるときは、互換範囲の唯一の例外として、その一方を前の版に固定してください。もう一方が追いついた時点で、固定を外します。

## 依存ライブラリのライセンス表示

サーバーの各リリースアーカイブには `THIRD_PARTY_LICENSES.txt` が入っています。内容は、その配布対象でサーバーが依存するライブラリのライセンスと、それぞれのソースの取得先です。`xtask` は梱包のときにこのファイルを生成し、リポジトリには置きません。生成には、[cargo-about](https://github.com/EmbarkStudios/cargo-about) 0.9.2を、`about.toml` とテンプレート `xtask/notices.hbs` で実行します。このバージョンは次のコマンドで導入します。

```sh
cargo install --locked --features cli cargo-about@0.9.2
```

`about.toml` には、受け入れるライセンスを列挙しています。そのどれも選べない依存があると、`check-notices` と梱包が失敗します。ライセンスを確認してから、一覧に追加してください。

`THIRD_PARTY_LICENSES.txt` には、各ライセンスを、クレートの作者が書いた文面で、著作権表示とともに載せます。cargo-aboutがクレートのライセンスファイルを自分で見つけられない場合は、`about.toml` にそのファイルを書いています。SHA-256チェックサムも書いています。

ライセンスファイルを同梱していないクレートについては、`about.toml` にGitHub上のそのクレートのリポジトリにあるファイルを指定しています。cargo-aboutは、生成のたびに、クレートが公開された時点のコミットからそのファイルを取得します。そのため、生成にはネットワーク接続が必要です。

チェックサムが合わないときと取得に失敗したとき、cargo-aboutは著作権者の名前がない標準の文面を使い、成功として終了します。この場合、`xtask` は該当するクレートの名前を挙げて失敗します。取得に失敗したときは、その前にcargo-aboutを合計3回まで実行し直します。標準の文面のままにしてよいクレートは、理由とともに `xtask/src/notices.rs` の `STANDARD_TEXT` に列挙しています。`THIRD_PARTY_LICENSES.txt` では、その文面の前にその旨を書きます。

cargo-aboutは、`about.toml` にライセンスを書いたクレートについて、クレートが宣言するライセンスの代わりにその記述を使います。チェックサムが確かめるのは、そこに指定したファイルだけです。そのため `xtask` は、クレートの宣言が `about.toml` の記述と異なるときにも失敗します。意図して宣言と異なる記述にしているクレートは、`xtask/src/notices.rs` の `DECLARED` に列挙しています。その記述を書いたときの宣言も添えています。

依存を更新したあとに `check-notices` が失敗したら、名前が挙がったクレートのライセンスファイルを読んでください。`about.toml` を変えるのは、そのあとです。次のようにします。

- ファイルが変わっていた場合は、変更の内容を確認してから、チェックサムを新しいファイルのSHA-256チェックサムに更新します。
- `about.toml` にそのクレートがない場合は、ライセンスファイルを指定する項目を追加します。
- クレートの宣言が変わっていた場合は、クレートに追加されたライセンスファイルがないかも確認してから、`about.toml` の項目を更新します。意図して宣言と異なる記述にしている項目では、`DECLARED` も更新します。

クレートのファイルは、Cargoのレジストリのソースディレクトリにあります。リポジトリのファイルは `https://raw.githubusercontent.com/<owner>/<repository>/<commit>/<path>` にあります。コミットは、クレートの `.cargo_vcs_info.json` に記録されています。`ra-ap-rustc_lexer` はそこにコミットを記録していません。クレートの説明文に書かれたコミットを `override-git-commit` に写してください。

cargo-aboutのバージョンを変えるときは、`xtask/src/notices.rs` と2つのワークフローの両方を変えてください。
