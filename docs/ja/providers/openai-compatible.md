# OpenAI互換API

[English](../../providers/openai-compatible.md)

`openai_compatible` プロバイダーを選ぶと、OpenAI APIのChat Completionsの要求を受け付けるサービスで翻訳します。OpenRouter、Gemini API、Ollama、LM Studioなどがこれに当たります。

サービスは `base_url` で指定し、要求はそこに送ります。APIキーは、コードアクションの `OpenAI-compatible API key…` からそのサービス用に保存した場合にだけ一緒に送ります。

## サービスごとの設定

以下の節から、使うサービスの `provider` を選んでください。Zedの設定の `lsp` → `translation-lsp` → `initialization_options` の中に書きます。`initialization_options` 内のほかの項目は、[設定](../configuration.md)で説明するとおりに使えます。

各節の `base_url` は各サービスの資料にある値です。モデルは例で、モデル名は送り先のサービスでの名前を使います。

### Gemini API

```json
"provider": {
  "name": "openai_compatible",
  "base_url": "https://generativelanguage.googleapis.com/v1beta/openai/",
  "model": "gemini-3.8-flash"
}
```

Gemini APIのキーを、[認証](#認証)のとおりに保存してください。

### OpenRouter

```json
"provider": {
  "name": "openai_compatible",
  "base_url": "https://openrouter.ai/api/v1",
  "model": "google/gemini-3.8-flash"
}
```

OpenRouterのAPIキーを保存してください。OpenRouterのモデル名は、`google/gemini-3.8-flash` のように、モデルの提供元の名前で始まります。

### お使いのコンピューターのOllama

```json
"provider": {
  "name": "openai_compatible",
  "base_url": "http://localhost:11434/v1",
  "model": "MODEL"
}
```

`MODEL` は、Ollamaに取り込んだモデルの名前に置き換えます。`ollama list` に出る名前です。キーは要りません。

PLaMo 3 Translateには、代わりに[PLaMo Translate](plamo-translate.md)のプロバイダーを選びます。このプロバイダーが送る指示文に、このモデルは従いません。

### お使いのコンピューターのLM Studio

```json
"provider": {
  "name": "openai_compatible",
  "base_url": "http://localhost:1234/v1",
  "model": "MODEL"
}
```

`MODEL` は、LM Studioに読み込んだモデルの識別子に置き換えます。キーは要りません。

### キーはサービスごとに別

あるサービス用に保存したキーは、ほかのサービスには送りません。たとえば、Gemini APIのキーはOpenRouterには送りません。`base_url` を別のサービスに変えたら、そのサービスがキーを必要とする場合は、そのサービスのキーを保存してください。キーなしで送った要求をサービスが拒否した場合、エラーには、キーが保存されていない送り先を表示します。キーの保存のしかたは[認証](#認証)で説明します。

## 設定項目

| 設定 | 既定値 | 規則 |
| --- | --- | --- |
| `provider.name` | なし | `"openai_compatible"` でこのプロバイダーを選びます。 |
| `provider.base_url` | なし | 必須です。要求の送り先のURLで、この後に `/chat/completions` を付けて送ります。`https://` で始まる必要があります。お使いのコンピューターで動くサービスに限り、`http://` も使えます。`localhost`、`127.0.0.1` から `127.255.255.255` までのアドレス、`::1` がこれに当たります。ユーザー名、パスワード、クエリ、フラグメントを含む値と、2,048バイトを超える値は設定エラーです。末尾の `/` は無視します。 |
| `provider.model` | なし | 必須です。空の値、空白や制御文字を含む値、256バイトを超える値は設定エラーです。指定した値をそのまま送り、別のモデルへ置き換えません。 |
| `provider.effort` | なし | 省略できます。指定した値を、要求の `reasoning_effort` としてそのまま送ります。省略した場合は `reasoning_effort` を送りません。空の値、空白や制御文字を含む値、64バイトを超える値は設定エラーです。 |
| `provider.document` | なし | 省略できます。タブに開く翻訳で使うモデルと設定を入れるオブジェクトです。中の値は `provider.model` と `provider.effort` を引き継ぎません。中で省略した値は送りません。タブ翻訳も同じ `base_url` へ送ります。中の未知の項目は設定エラーです。 |
| `provider.document.model` | なし | `provider.document` を書く場合は必須です。規則は `provider.model` と同じです。 |
| `provider.document.effort` | なし | 省略できます。規則は `provider.effort` と同じです。 |

`reasoning_effort` に使える値はサービスごとに違うため、サーバーは値の形だけを検査します。Gemini APIの資料によると、`minimal`、`low`、`medium`、`high` などの値を受け付けます。それぞれをモデルの推論の段階に置き換えます。Ollamaの資料によると、使える値はモデルによって違います。

未知の項目は拒否します。APIキーを指定する設定項目はありません。設定を変更したら言語サーバーを再起動してください。

### タブ翻訳だけ別のモデルを使う

```json
"provider": {
  "name": "openai_compatible",
  "base_url": "https://openrouter.ai/api/v1",
  "model": "google/gemini-3.8-flash",
  "document": { "model": "anthropic/claude-sonnet-5.5" }
}
```

`provider.document` の中に書ける値は、上の表にあります。上のモデルは例です。

`provider.document` を設定したときにどのプロバイダーでも変わる点は、[タブ翻訳だけ別のモデルを使う](../configuration.md#タブ翻訳だけ別のモデルを使う)で説明します。

## 認証

お使いのコンピューターで動くOllamaやLM Studioなど、キーの要らないサービスは、キーなしで使えます。キーを保存するまで、要求に `Authorization` ヘッダーは付けません。

キーは次の手順で保存します。

1. 対応する文書でZedのコマンドパレットから `editor: toggle code actions` を実行します。メニューから `OpenAI-compatible API key…` を選びます。
2. `Save OpenAI-compatible API key` を実行します。OSの端末のウィンドウが開き、キーの送り先のアドレスを示して、キーを尋ねます。
3. キーを貼り付けてEnterを押します。貼り付けた内容は端末に表示されません。キーを入れずにEnterを押すと、何も保存しません。
4. 保存した旨が表示されたら、Enterを押してZedに戻ります。次の翻訳から、キーを `Authorization: Bearer` として送ります。言語サーバーの再起動は要りません。

キーが保存されている間、メニューは次の項目に変わります。

- `Replace OpenAI-compatible API key` は、同じ端末を開き、保存済みのキーを別のキーに置き換えます。
- `Remove OpenAI-compatible API key` は、確認のあと、お使いのコンピューターからこのサービスのキーのファイルを削除します。進行中の翻訳は取り消し、表示済みの訳文は破棄します。以後の要求にはキーを付けません。キーそのものは、サービスで削除するまで有効です。

メニューはMarkdownとPlain Textの文書に出ます。ほかの文書では、キーなしの要求をサービスが拒否したときなど、翻訳に失敗した範囲にだけ出ます。それ以外のときにキーを保存、置き換え、削除するには、MarkdownかPlain Textのファイルを開いてください。

端末の開き方は、Anthropic APIのキーと同じで、そのページの[端末のウィンドウが開かないとき](anthropic.md#端末のウィンドウが開かないとき)で説明します。`Save OpenAI-compatible API key` と `Replace OpenAI-compatible API key` のあとには通知が出ます。通知には、ウィンドウが開かないときに自分で実行するコマンドが入っています。言語サーバーのパスに、`api-key` とサービスのアドレスを続けたものです。例: `api-key https://openrouter.ai`。

キーは、`base_url` のスキーム、ホスト、ポートの組に属し、そこにだけ送ります。`base_url` を別のサービスに変えると、そのサービスのキーを使い、なければキーなしで送ります。ほかのサービスのキーは、それぞれのファイルに残ります。

キーは組ごとに、サーバーの認証情報のディレクトリに別のファイルを持ちます。ディレクトリとファイルの検査は、[Anthropicのキーのファイル](anthropic.md#キーのファイル)と同じです。名前は、`api-key-` に、スキーム、ホスト、ポートを `-` でつないで続けたものです。例えば `api-key-https-openrouter.ai-443` や `api-key-http-localhost-11434` です。IPv6アドレスは、コロンを `_` に置き換えて角括弧で囲み、`api-key-http-[__1]-8080` のように書きます。

| OS | 認証情報のディレクトリ |
| --- | --- |
| Windows | `%LOCALAPPDATA%\translation-lsp\` |
| macOS | `~/Library/Application Support/translation-lsp/` |
| Linux | `$XDG_CONFIG_HOME/translation-lsp/`。未設定の場合は `~/.config/translation-lsp/` |

[OpenAI API](openai-api.md)のプロバイダーはファイル `api-key-https-api.openai.com-443` を使います。そのため、`https://api.openai.com` のキーは2つのプロバイダーで共有します。

キーは、端末からファイルへ渡ります。Zedの設定には書かず、環境変数も、Zedとサーバーの間のメッセージも通りません。

キーのファイルが空のとき、キー以外のものを含むとき、権限が緩いときは、使えないファイルとして、直すか削除するまで翻訳を設定エラーで止めます。

## 費用と使用量

サーバーは使用量や費用を数えません。サービスで確認してください。`provider.document` を設定すると、タブ翻訳にはそこで指定したモデルを使います。

コメントのホバーとMarkdownの選択による自動翻訳は、[使い方](../usage.md)のとおり、確認なしに要求を送ります。

お使いのコンピューターで動くモデルは、既定の期限より時間がかかることがあります。翻訳が期限切れになる場合は、`hover_timeout_ms`、`selection_timeout_ms`、`document_timeout_ms` を大きくしてください。[タイミング](../configuration.md#タイミング)を参照してください。

## 送信する内容

翻訳のために送る文章は、[送信する内容](../usage.md#送信する内容)で説明します。

要求は、ストリームで返すChat Completionsの要求で、メッセージを2件含めます。システムメッセージとして翻訳の指示文を、ユーザーメッセージとして翻訳する文章を含めます。単語の意味を調べる場合は、代わりに単語の説明を求める指示文と、語と、語のある行を含めます。会話履歴は送りません。

要求の出力は、次のように抑えます。

- ホバーで読む翻訳では、`max_tokens` として、16,384に原文（意味を調べる場合は語のある行）1バイトあたり4を足した値を送ります。`translation_limits.output_kib` をバイト数に換算した値（1 KiBあたり1,024）がそれより小さい場合は、その値を送ります。コメントの中の文章によって、モデルに長い出力を書かせないためです。
- タブに開く翻訳は `max_tokens` を送りません。モデル自身の上限を超える値は拒否されることがあるためです。タブ翻訳は `translation_limits.output_kib` で抑えます。

訳文は、応答の最初の選択肢の本文です。サービスが専用の項目で返す推論は表示しません。モデルが応答の本文に書いた推論は、訳文の一部として表示します。

[`document_split`](../configuration.md#文書を分けて送る) を有効にすると、別タブに開く翻訳を部分ごとに1回の要求で、いくつかを同時に送ることがあります。部分の要求に加える指示は、[送信する内容](../usage.md#送信する内容)で説明します。どの要求もここで説明した形で、別タブに開く翻訳の上限で送ります。

## エラーと停止

応答は、`finish_reason` が `stop` で完了したときだけ表示します。次の応答は、途中までの内容を表示せずにエラーとして報告します。トークンの上限で終わったもの、コンテンツフィルターで止められたもの、ほかの理由か理由なしで終わったものです。

応答が始まったあとにサービスが送ったエラーは、ストリームの失敗として報告します。OpenRouterのようにエラーコードが数値の場合は、それをHTTPの状態として扱います。

| エラー | 起きること | すること |
| --- | --- | --- |
| 残高の不足（HTTP 402）、権限のエラー（403）、存在しないモデルやアドレス（404） | 自動翻訳を停止します。エラーは、HTTPの状態コードを、サービスが返した場合はエラーコードと要求IDも報告します。 | 原因を解消します。そのあと、選択範囲または文書の翻訳を明示的に実行してください。それが成功すると自動翻訳を再開します。 |
| 拒否されたキーやキーがないこと（HTTP 401） | 自動翻訳を停止し、同じように報告します。要求の送り先と、そこにキーが保存されているかどうかも示します。 | `OpenAI-compatible API key…` からキーを保存するか、拒否されたキーの代わりに別のキーを保存すると、停止は終わります。 |
| 時間切れ（408）、レート制限（429）、サーバーのエラー | [自動の再試行](../behavior.md#自動の再試行)のとおり一度だけ再試行します。ストリームの中で報告されたものや、コードが `rate_limit_exceeded`、`server_error`、`server_is_overloaded` のものも含みます。 | — |
| コンテンツフィルターによる停止 | 失敗とします。再試行せず、自動翻訳も停止しません。 | — |

キーの保存、置き換え、削除に、言語サーバーの再起動は要りません。あるキーで、またはキーなしで得た訳文と失敗の記録は、別のキーでは表示しません。キーが替わったときに進行中だった翻訳は、完了しても破棄します。設定を変更した場合は、言語サーバーの再起動が必要です。

どのプロバイダーにも共通する規則は、[停止](../behavior.md#停止)と[自動の再試行](../behavior.md#自動の再試行)で説明します。

## 保存するデータ

キーは、[認証](#認証)で説明したファイルに、平文で入ります。ChatGPTプロバイダーが認証情報を保存するのと同じディレクトリです。ファイルを書くのは、メニューが端末で動かす `api-key` コマンドです。ディレクトリがなければ、このコマンドが作ります。`Remove OpenAI-compatible API key` は、いま使っているサービスのファイルだけを削除します。それ以外では、言語サーバーはファイルを読むだけです。

macOSで「ターミナル」を開くとき、サーバーは一時ディレクトリにスクリプトを書きます。スクリプトに入るのは言語サーバーのパスとサービスのアドレスだけで、キーは入りません。「ターミナル」が実行すると、スクリプトは自分を削除します。

サーバーがキーをログ、エラー文、通知、ディスクのほかの場所に書くことはありません。
