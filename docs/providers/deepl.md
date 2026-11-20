# DeepL API

[日本語](../ja/providers/deepl.md)

Select the `deepl` provider to translate through the DeepL API with an API key, and save the key from the `DeepL API key…` code action. The provider works with keys of DeepL API Free and of the paid DeepL API plans.

DeepL is a translation service, not a language model: it takes no instructions. The server therefore prepares the text itself, as described under [What is sent](#what-is-sent), and a selected word is translated instead of being looked up in a dictionary entry.

## Minimal settings

```json
{
  "lsp": {
    "translation-lsp": {
      "initialization_options": {
        "target_language": "ja",
        "provider": {
          "name": "deepl"
        }
      }
    }
  }
}
```

The other settings in `initialization_options` work as described under [Configuration](../configuration.md). The API key is not part of the settings; save it as described under [Authentication](#authentication).

## Settings

| Setting | Default | Rules |
| --- | --- | --- |
| `provider.name` | None | `"deepl"` selects this provider. |

The provider has no other fields. Unknown fields, such as `model` or `document`, are settings errors.

With this provider, `target_language` must be a language code, such as `ja`, `en-US`, `pt-BR`, or `zh-Hans`, and `source_language` must be `auto` or a language code. A language name such as `Japanese` is a settings error. With `auto`, DeepL identifies the language.

DeepL decides which languages it translates into, and which regional variants it needs, such as `en-US` or `en-GB` for English. A code it does not take is reported as an error with DeepL's message, such as `Value for 'target_lang' not supported.` DeepL lists its languages on its [supported languages](https://developers.deepl.com/docs/getting-started/supported-languages) page.

Restart the language server after changing these settings.

## Authentication

Find the key on the [API Keys & Limits page](https://www.deepl.com/your-account/keys) of your DeepL account.

To save the key:

1. In a supported document, run `editor: toggle code actions` from Zed's Command Palette and choose `DeepL API key…`.
2. Run `Save DeepL API key`. A terminal window of the OS opens and asks for the key.
3. Paste the key and press Enter. The terminal does not show what you paste. Pressing Enter without a key saves nothing.
4. When the terminal reports that the key was saved, press Enter and return to Zed. The next translation uses the key, without a language server restart.

A key of DeepL API Free, which ends in `:fx`, is sent to `https://api-free.deepl.com`; any other key is sent to `https://api.deepl.com`. There is no setting for the address.

While no usable key is saved, `DeepL API key…` is the only code action of the server. While one is saved, the menu has these items instead:

- `Replace DeepL API key`: open the same terminal to save another key in place of the saved one.
- `Remove DeepL API key`: after you confirm, delete the key file from your computer, cancel the translations in progress, and discard the translations already shown. The key stays valid until you delete it in your DeepL account.

While a key is saved, the menu appears in Markdown and Plain Text documents, and in other documents only in a range whose translation failed, such as with a rejected key. To replace or remove the key at another time, open a Markdown or Plain Text file.

The terminal opens as for the Anthropic API key, described under [When no terminal window opens](anthropic.md#when-no-terminal-window-opens) of that page. The notice that follows `Save DeepL API key` or `Replace DeepL API key` shows the command to run by hand when no window appears. The command is the path of the language server, then `deepl-key`.

The key is saved in the file `deepl-api-key` in the server's credential directory, with the same checks of the directory and the file as the [Anthropic key file](anthropic.md#key-file).

| OS | Key file |
| --- | --- |
| Windows | `%LOCALAPPDATA%\translation-lsp\deepl-api-key` |
| macOS | `~/Library/Application Support/translation-lsp/deepl-api-key` |
| Linux | `$XDG_CONFIG_HOME/translation-lsp/deepl-api-key`, or `~/.config/translation-lsp/deepl-api-key` when unset |

The key reaches the file from the terminal. It is not written in the Zed settings and does not pass through an environment variable or through the messages between Zed and the server.

Without a usable key the server still starts, and a translation request returns a settings error without contacting the API.

## Cost and usage

DeepL counts the characters of the text it translates against the plan of the account that owns the key: a monthly allowance with DeepL API Free, and a charge with the paid plans. The plans and prices are on DeepL's pricing page. Translations opened in a tab are sent and counted like any other.

Automatic comment Hover and Markdown selection translation send requests without a confirmation, as described under [Usage](../usage.md).

The `DeepL API key…` menu saves, replaces, and removes the key. It has no usage command; usage is shown on the [API Usage page](https://www.deepl.com/your-account/usage) of your DeepL account.

## What is sent

The text submitted for translation is described under [What is sent](../usage.md#what-is-sent). Because DeepL takes no instructions, the server sends that text in segments, which DeepL translates independently of each other:

- The lines of a paragraph are joined into one segment, so that a sentence broken across lines is translated as one sentence. A translated paragraph is one line, however many lines the original had.
- A Markdown heading, a list item, a quote, and a table cell are each a segment. The marks that began the line, such as `## `, `- `, or `> `, are kept.
- Code blocks between fences such as `` ``` `` are not sent and are kept as they were. A segment with no letters outside its code spans and URLs is not sent either.
- Code spans between backticks and URLs starting with `http://` or `https://` are sent marked, and DeepL is asked to leave the marked parts untranslated. The server puts the original text back in place of each mark.
- Indented lines are not read as code and are translated. Put code that must not be translated in a fenced code block or a code span.

Each request goes to `/v2/translate` at the address of the key, described under [Authentication](#authentication). One request holds at most 96 KiB of segments. A longer text is sent in several requests, one after another, and is shown only when every request has succeeded.

With `source_language` set to `auto`, DeepL reports the language it detected in each segment, which decides whether comment Hover and automatic Markdown selection show anything, as described under [Text already in the target language](../usage.md#text-already-in-the-target-language). DeepL reports Chinese as `ZH`, which names neither script, so it matches only the target language `zh`, not `zh-Hans` or `zh-Hant`.

With [`document_split`](../configuration.md#document-split) enabled, a translation opened in a tab can be sent in parts, several at once, and each part is sent in segments as described here, in requests of its own.

## Errors and pauses

| Error | What happens | What to do |
| --- | --- | --- |
| An exhausted quota (HTTP 456), or a language code that DeepL does not take | Automatic translation stops. The error reports the HTTP status and DeepL's message. | Check the usage of the account, or correct the settings. Then run a selection or document translation explicitly; automatic translation resumes when it succeeds. |
| A rejected key (HTTP 403) | Automatic translation stops for that key, with the same report. | Save another key from `DeepL API key…`; the stop ends. |
| Too many requests (HTTP 429 and 529) or a server error | The request is retried once, as described under [Automatic retries](../behavior.md#automatic-retries). | — |
| DeepL lost, repeated, or added a mark | The translation is reported as an error instead of being shown. It is not retried and does not pause automatic translation. | — |

Saving, replacing, or removing the key needs no language server restart. Translations and failures kept for one key are not shown for another, and a translation that is in progress when the key changes is discarded when it completes. Changing a setting requires a language server restart.

The rules shared by every provider are described under [Pauses](../behavior.md#pauses) and [Automatic retries](../behavior.md#automatic-retries).

## Stored data

The key is stored as plain text in the file `deepl-api-key` described under [Authentication](#authentication), in the directory where the ChatGPT provider stores its credentials. The `deepl-key` command, which the menu runs in a terminal, writes the file, and creates the directory when it is missing. `Remove DeepL API key` deletes the file and nothing else. Otherwise the language server only reads the file.

To open Terminal on macOS, the server writes a script to the temporary directory. The script holds the path of the language server and no key, and deletes itself when Terminal runs it.

The server does not write the key to logs, error messages, notices, or anywhere else on disk.
