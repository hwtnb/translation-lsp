# PLaMo Translate

[English](../../providers/plamo-translate.md)

`plamo_translate` プロバイダーを選ぶと、Preferred Networksの翻訳用のモデルPLaMo 3 Translateで翻訳します。モデルは、Ollamaなど、お使いのコンピューターでモデルを動かす道具を通じて使います。拡張機能はモデルも道具も含まないため、ご自身で入れてください。モデルの大きさ、入手の方法、ライセンスは、[モデルカード](https://huggingface.co/pfnet/plamo-3-translate-2b-GGUF)にあります。

このモデルは翻訳のためのもので、指示文を受け付けません。そのため、[送信する内容](#送信する内容)のとおり、サーバーは文章を書かれたとおりに送ります。また、選択した単語は、意味を調べずに翻訳します。

## Ollamaでモデルを動かす

1. [Ollama](https://ollama.com)を入れて、起動します。
2. モデルを取り込みます。モデルカードの2Bのモデルなら、次のとおりです。

   ```sh
   ollama pull hf.co/pfnet/plamo-3-translate-2b-GGUF:Q4_K_M
   ```

3. Zedの設定の `lsp.translation-lsp.initialization_options` に、プロバイダーを書きます。

   ```json
   {
     "lsp": {
       "translation-lsp": {
         "initialization_options": {
           "target_language": "ja",
           "provider": {
             "name": "plamo_translate",
             "base_url": "http://localhost:11434/v1",
             "model": "hf.co/pfnet/plamo-3-translate-2b-GGUF:Q4_K_M"
           }
         }
       }
     }
   }
   ```

   `model` は、`ollama list` に出るモデルの名前です。キーは要りません。

4. この拡張機能を使う文書で、コマンドパレットから `editor: restart language server` を実行します。

`initialization_options` のほかの設定は、[設定](../configuration.md)のとおりに働きます。

### LM Studio

モデルカードによると、LM Studioは `http://localhost:1234/v1` で要求を受け付け、`model` は読み込んだモデルの識別子です。

```json
"provider": {
  "name": "plamo_translate",
  "base_url": "http://localhost:1234/v1",
  "model": "plamo-3-translate-2b"
}
```

## 設定項目

| 設定 | 既定値 | 規則 |
| --- | --- | --- |
| `provider.name` | なし | `"plamo_translate"` でこのプロバイダーを選びます。 |
| `provider.base_url` | なし | 必須です。要求は、このURLの後ろに `/chat/completions` を付けたところへ送ります。[OpenAI互換API](openai-compatible.md#設定項目)の `provider.base_url` と同じ規則です。 |
| `provider.model` | なし | 必須です。OpenAI互換APIの `provider.model` と同じ規則です。値は書かれたとおりに送ります。 |

このプロバイダーには、ほかの項目はありません。`effort` や `document` など、知らない項目は設定エラーです。タブ翻訳も同じモデルを使います。

このプロバイダーでは、`target_language` は `ja`、`en`、`zh-Hans` のような言語コードにする必要があります。`source_language` は `auto` か言語コードです。`Japanese` のような言語名は設定エラーです。モデルが受け付けるコードは、モデルカードのSupported Languagesにあり、大文字と小文字は区別されません。サーバーはコードをその一覧と照らし合わせず、書かれたとおりに送ります。

一覧にないコードは、拒まれるか、別の言語に訳されます。Ollamaでは、`en-US` や `en-us` のように地域を付けたコードはHTTP 400で拒まれ、そのメッセージはモデルのチャットテンプレートのエラーで終わります。`pt` は拒まれませんが、応答は別の言語で返り、その言語は文章によって違います。

設定を変更したら、言語サーバーを再起動してください。

## 認証

Ollamaの既定のように、キーを求めない設定の道具には、キーは要りません。キーを求めるサービスや設定では、OpenAI互換APIのページの[認証](openai-compatible.md#認証)のとおり、コードアクションの `OpenAI-compatible API key…` からキーを保存します。2つのプロバイダーはメニューとキーのファイルを共有するため、同じアドレスには同じキーを送ります。

## 費用と使用量

要求は `base_url` にだけ送ります。サーバーは使用量を数えません。

コメントのホバーとMarkdownの選択の自動翻訳は、確認なしに要求を送ります。[使い方](../usage.md)で説明します。

お使いのコンピューターのモデルは、既定の期限より長くかかることがあります。翻訳が期限切れになるときは、`hover_timeout_ms`、`selection_timeout_ms`、`document_timeout_ms` を大きくしてください。[タイミング](../configuration.md#タイミング)を参照してください。

## 送信する内容

翻訳のために送る文章は、[送信する内容](../usage.md#送信する内容)で説明します。要求は、ストリーミングするChat Completionsの要求で、2つのメッセージを持ちます。

- システムメッセージには、モデルの設定だけを書きます。`[plamo-translate]` の行のあと、`target_lang=` に続けて `target_language` を、`source_language` が言語コードのときは `source_lang=` に続けてそのコードを、1行に1つずつ書きます。
- ユーザーメッセージは、訳す文章を書かれたとおりに送ります。区切りも印も付けません。コード、URL、Markdownもそのまま送ります。

要求には、モデルカードの例のとおり、`temperature` の0と、モデルの特別なトークン `<|plamo:tag|>` と `<|plamo:bos|>` で止める指定も付けます。会話履歴は送りません。最初の選択肢の応答を、そのまま訳文にします。

出力の上限は、OpenAI互換APIと同じです。ホバーで読む翻訳は `max_tokens` を送り、別タブに開く翻訳は送りません。そのページの[送信する内容](openai-compatible.md#送信する内容)を参照してください。

[`document_split`](../configuration.md#文書を分けて送る) を有効にすると、別タブに開く翻訳を部分ごとに1つの要求で送ることがあります。どの要求も、ここで説明した形です。

### 翻訳先の言語ですでに書かれた文章

このモデルは言語を示しません。コメントのホバーとMarkdownの自動の選択では、`source_language` が `auto` のとき、応答が両端の空白を除いて送った文章と同じなら、サーバーは何も表示しません。モデルがそのような文章を少し書き換えて返すこともあり、そのときは応答を訳文として表示します。[翻訳先の言語ですでに書かれた文章](../usage.md#翻訳先の言語ですでに書かれた文章)を参照してください。

### 長い文章

文章と訳文は、合わせて、モデルを動かす道具のコンテキストに収まる必要があります。超える要求は道具のエラーで失敗し、訳文は一部も表示せず、残しもしません。

Ollamaでは、エラーは `PLaMo Translate HTTP 400: request (… tokens) exceeds the available context size` のように、要求の数を添えた文で始まります。

もっと長い文章を訳すには、道具のコンテキストの長さを上げるか、[`document_split`](../configuration.md#文書を分けて送る) を有効にして、各部分が収まるまで `chunk_kib` を下げます。各部分とその訳文が1つの要求になり、収まらない部分があると、ほかの部分の失敗と同じく、翻訳全体が失敗します。

Ollamaは、コンテキストの長さを、`ollama serve` の環境変数 `OLLAMA_CONTEXT_LENGTH` か、Ollamaのアプリの設定から取ります。長くするほど、メモリーを多く使います。Ollamaの資料の[Context length](https://docs.ollama.com/context-length)を参照してください。

## エラーと停止

エラーと停止は、OpenAI互換APIのページの[エラーと停止](openai-compatible.md#エラーと停止)と同じです。エラーには、サービスとして `PLaMo Translate` と出ます。

道具が動いていないときは、`PLaMo Translate network request failed or timed out; retry later` というエラーになります。

## 保存するデータ

このプロバイダーは、独自のデータを保存しません。キーはOpenAI互換APIのもので、そのページの[保存するデータ](openai-compatible.md#保存するデータ)で説明します。
