# PLaMo Translate

[日本語](../ja/providers/plamo-translate.md)

Select the `plamo_translate` provider to translate with PLaMo 3 Translate, a translation model of Preferred Networks, through a tool that runs it on your computer, such as Ollama. The extension includes neither the model nor the tool; install them yourself. The sizes of the model, how to download it, and its license are on its [model card](https://huggingface.co/pfnet/plamo-3-translate-2b-GGUF).

The model is made for translation and takes no instructions. The server therefore sends the text as written, as described under [What is sent](#what-is-sent), and a selected word is translated instead of being looked up in a dictionary entry.

## Running the model with Ollama

1. Install [Ollama](https://ollama.com) and start it.
2. Download the model. For the 2B model of the model card:

   ```sh
   ollama pull hf.co/pfnet/plamo-3-translate-2b-GGUF:Q4_K_M
   ```

3. Write the provider in `initialization_options` under `lsp` → `translation-lsp` in the Zed settings:

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

   `model` is the name of the model as `ollama list` shows it. No key is needed.

4. In a document using this extension, run `editor: restart language server` from the Command Palette.

The other settings in `initialization_options` work as described under [Configuration](../configuration.md).

### LM Studio

According to the model card, LM Studio takes the requests at `http://localhost:1234/v1`, and `model` is the identifier of the loaded model:

```json
"provider": {
  "name": "plamo_translate",
  "base_url": "http://localhost:1234/v1",
  "model": "plamo-3-translate-2b"
}
```

## Settings

| Setting | Default | Rules |
| --- | --- | --- |
| `provider.name` | None | `"plamo_translate"` selects this provider. |
| `provider.base_url` | None | Required. The URL that requests go to with `/chat/completions` after it, with the rules of `provider.base_url` of the [OpenAI-compatible API](openai-compatible.md#settings). |
| `provider.model` | None | Required. The rules of `provider.model` of the OpenAI-compatible API apply. The value is sent as written. |

The provider has no other fields. Unknown fields, such as `effort` or `document`, are settings errors. Tab translations use the same model.

With this provider, `target_language` must be a language code, such as `ja`, `en`, or `zh-Hans`, and `source_language` must be `auto` or a language code. A language name such as `Japanese` is a settings error. The codes that the model takes are listed under Supported Languages on its model card, and their case does not matter. The server sends the code as written, without checking it against that list.

A code outside the list is refused or translated into another language. With Ollama, a code with a region, such as `en-US` or `en-us`, is refused with HTTP 400, and the message ends with an error of the model's chat template. The code `pt` is not refused, and the reply comes in another language, which differs from text to text.

Restart the language server after changing these settings.

## Authentication

A tool set up to ask for no key, as Ollama is by default, needs none. For a service or a setup that asks for a key, save it from the `OpenAI-compatible API key…` code action, as described under [Authentication](openai-compatible.md#authentication) of the OpenAI-compatible API page. The two providers share the menu and the key files, so they send the same key to the same address.

## Cost and usage

Requests go to `base_url` only. The server does not count usage.

Automatic comment Hover and Markdown selection translation send requests without a confirmation, as described under [Usage](../usage.md).

A model on your computer may take longer than the default deadlines. Raise `hover_timeout_ms`, `selection_timeout_ms`, or `document_timeout_ms` when translations time out; see [Timing](../configuration.md#timing).

## What is sent

The text submitted for translation is described under [What is sent](../usage.md#what-is-sent). Each request is a streamed Chat Completions request with two messages:

- The system message holds only the settings of the model: the line `[plamo-translate]`, then `target_lang=` with `target_language`, and `source_lang=` with `source_language` when it is a code, one setting a line.
- The user message is the text to translate, as written. It is not divided or marked: code, URLs, and Markdown are sent as they are.

The request also sends `temperature` 0, and stops at the model's own tokens `<|plamo:tag|>` and `<|plamo:bos|>`, as the model card shows. Conversation history is not sent. The reply of the first choice is the translation, as it comes.

The output is limited as for the OpenAI-compatible API: a translation read in Hover sends `max_tokens`, and a translation opened in a tab sends none. See [What is sent](openai-compatible.md#what-is-sent) of that page.

With [`document_split`](../configuration.md#document-split) enabled, a translation opened in a tab can be sent as one request for each part, each of the form described here.

### Text already in the target language

The model names no language. For comment Hover and automatic Markdown selection with `source_language` set to `auto`, the server shows nothing when the reply is the text sent, apart from whitespace at its start and end. The model may also return such text slightly rewritten; the server then shows the reply as a translation. See [Text already in the target language](../usage.md#text-already-in-the-target-language).

### Long texts

The text and its translation together must fit in the context of the tool that runs the model. A request beyond it fails with the error of the tool, and no part of the translation is shown or kept.

With Ollama, the error starts with `PLaMo Translate HTTP 400: request (… tokens) exceeds the available context size`, with the numbers of the request.

To translate a longer text, raise the context length of the tool, or enable [`document_split`](../configuration.md#document-split) and lower `chunk_kib` until each part fits. Each part and its translation are one request, and a part that does not fit fails the whole translation, as any failed part does.

Ollama takes the context length from the `OLLAMA_CONTEXT_LENGTH` environment variable of `ollama serve`, or from the settings of the Ollama app. A longer context needs more memory. See [Context length](https://docs.ollama.com/context-length) in the Ollama documentation.

## Errors and pauses

Errors and pauses are as for the OpenAI-compatible API, described under [Errors and pauses](openai-compatible.md#errors-and-pauses), and the errors name `PLaMo Translate` as the service.

When the tool is not running, the error reads `PLaMo Translate network request failed or timed out; retry later`.

## Stored data

The provider keeps no data of its own. The keys are those of the OpenAI-compatible API, described under [Stored data](openai-compatible.md#stored-data) of that page.
