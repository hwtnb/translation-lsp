# Usage

[日本語](ja/usage.md)

This page describes how to request, read, cancel, retry, and delete translations, and what text is sent. The rules for accepting, keeping, and caching work are described under [Behavior](behavior.md).

## Comment Hover

Hover over a supported code comment, or place the cursor in it and run Zed's `editor: hover`. Adjacent line comments of the same kind and indentation are translated together; blank lines or code split the group. Each block comment is handled separately. Comment markers and one following ASCII space are removed from the text submitted for translation.

A block comment or a docstring of more than one line is also sent without its margin:

- after `/*`, a first line of only `*` and `!`, such as the rest of `/**` or `/*!`
- in a comment that starts with `/*`, the indentation, leading `*`, and up to one following space or tab on each nonblank line after the first. This applies if every such line starts, after any indentation, with `*` followed by a space, a tab, or the end of the line
- the indentation that the following lines share, and the spaces and tabs at the start of the first line
- blank lines at the start and end, and spaces and tabs at the end of the last line

The lines keep their indentation relative to each other and the blank lines between them.

With `source_language` set to `auto`, a comment already written in the target language gets no Hover. The model or the translation service decides this, as described under [Text already in the target language](#text-already-in-the-target-language); the server does not guess the language itself. The server keeps that answer like a translation, so repeated Hover requests over the same text reuse it until it is deleted. `Translate` in the CodeLens and the Code Actions always translate the comment.

Zed controls Hover placement and width. The extension cannot guarantee placement above the text or that the popup leaves all source text visible.

## Markdown selection

With a provider ready, as described under [Set up a provider](../README.md#set-up-a-provider), and `features.automatic_selection` set to `true`, select Markdown prose and keep the selection stable. Translation starts in the background after the wait set by [`selection_debounce_ms`](configuration.md#timing); Hover over that range to read it. There is no background completion notification. With `source_language` set to `auto`, prose already written in the target language gets no Hover, as for [Comment Hover](#comment-hover); the Code Actions always translate it.

Selections containing only code, link destinations, or whitespace do not start translation. A fragment that changes the interpretation of Markdown by cutting through a code delimiter or another protected span is also excluded. Include the complete delimiters when selecting, or use the manual action.

Selecting prose just to copy it can also send it for translation.

## Manual selection translation

To translate immediately or recover from a background failure, use the `Translate selection to …` Code Action. It shows `Translation ready. Hover over the selected text to view it.` when complete. It works in code and every other supported document, and is hidden when nothing is selected.

The provider must be ready first. With the ChatGPT provider, connect or grant plan use through `ChatGPT account…`. With the Anthropic API or OpenAI API provider, save a key through `Anthropic API key…` or `OpenAI API key…`.

## Looking up a word

A selection on one line of up to three words and 64 characters, such as `retain`, `take over`, or the identifier `retain_completed`, is looked up instead of translated, unless it ends with `.`, `!`, `?`, `。`, `！`, or `？`. Hover over it to read a dictionary entry. The model writes the entry; the extension includes no dictionary data.

Words are counted by the Unicode word boundaries, which also divide languages written without spaces: in Japanese, `保持する` is two words and `設定を保存する` four, because particles and endings count as words.

An entry holds the following. Its labels, such as `Example:`, are in English; the meanings and parts of speech are in the target language.

- the term
- its meanings with their parts of speech, the meaning it has in its line first
- the words of an identifier or compound, and what they mean together
- one example sentence with its translation

Automatic Markdown selections, `Translate selection to …`, and `… and open in tab` look a term up. These do not:

- A selection of the whole document is translated, as `Translate document to … and open in tab` translates it.
- A comment read by Comment Hover is never looked up, even when it is one word.
- The DeepL API, Google Cloud Translation, and PLaMo Translate write no dictionary entries: with them, such a selection is translated like any other.

## How Hover shows a translation

Hover shows the title `Translation LSP` and, below a rule, the translation. With `hover.header` set to `false`, it shows only the translation. See [Hover](configuration.md#hover).

The translation of a comment, of a selection in code, or of Plain Text shows as Markdown that keeps this formatting of the translation:

- code spans, and code blocks fenced or indented after a blank line
- headings that start with `#` or are underlined with `===` or `---`
- lists that start with `-`, `*`, `+`, or a number, indented or not, block quotes, tables, and rules
- emphasis with `*` and `**`, and text struck out with `~~`
- links written as `[text](URL)` to a URL that the original text contains

Every other character shows as written: `__init__`, `--verbose`, `\d+\.\d+`, `Vec<T>`, and `<!-- note -->` stay as they are, and quotes and `...` are not replaced with typographic ones. Each line break of the translation shows as a line break, and one blank line or more separates paragraphs. Spaces at the start of a line inside a paragraph do not show. A translation whose formatting cannot be kept this way shows its lines as written, without formatting.

Links open only URLs that the original text contains: text in the source that addresses the model can make the translation contain links that the source does not have. A URL of the original ends before the punctuation of the sentence around it, such as `.` or an unmatched `)`. In the translation:

- A URL written alone is a link when it starts with a URL of the original and the text ends after it, or continues with whitespace or a character outside ASCII, possibly after punctuation. In `詳細はhttps://example.com/docsを参照`, `https://example.com/docs` is a link.
- A link written as `[text](URL)` or `<URL>` stays when its URL is exactly one of the original.
- A stretch of text without spaces that contains two URLs, such as a URL with another URL in its query, makes no link.

Every other URL shows as text with an invisible word joiner (U+2060) after its `:`, and text copied from Hover includes it. URLs inside code show as written and are not links.

With `hover.format` set to `"markdown"`, the translation of a Markdown document shows as its own Markdown. Its links and images stay when each destination is exactly one of the original, such as the same relative path, or starts with `#`.

If any other destination appears, or a URL that escapes or character references divide, such as `https&#58;//`, the translation shows in a code block as written instead. Its URLs written alone follow the rules above, and URLs in the text of an image are never links.

With `hover.format` set to `"text"`, every translation except a dictionary entry shows in a code block as written. A dictionary entry shows as described under [Looking up a word](#looking-up-a-word) with either value.

## Opening a translation in a tab

In Markdown or Plain Text, use `Translate selection to … and open in tab` to read the translation in a separate document. With no selection, the action becomes `Translate document to … and open in tab` and translates the whole document. Both operations use `translation_limits.input_kib.document`, which defaults to 1 MiB. The client must support `window/showDocument`.

A translation opened in a tab opens when it completes, within the time described under [Deadlines](behavior.md#deadlines); a long text gets no more time. Use the range or all-translations cancellation Code Actions to stop pending work. Editing or closing the source document also prevents the result from opening. With [`document_split`](configuration.md#document-split) enabled, a long text is sent in parts.

Markdown translations open as temporary `.md` files. Run `markdown: open preview` from the Command Palette to see the rendered document and return to its source tab when needed. Plain Text translations open as `.txt` files. The extension leaves your normal Markdown opening preference unchanged. Existing translation tabs are snapshots. Edit the original and run the action again to obtain a new translation.

Use **Save As** to keep a translation before restarting the language server or closing Zed. The server removes its temporary files on normal shutdown; an abnormal exit can leave them in the system temporary directory. Saved copies are yours to keep.

Relative links and images resolve from the temporary file location. To use the original relative paths, save the translation beside the source.

A translation is model output. Text in the source that addresses the model can make the translation contain links and images that the source does not have. `markdown: open preview` loads images from their `http` and `https` URLs as it renders them, so the server named in such a URL can receive any text of the translation placed in the URL. When someone else wrote the source, read the translation tab as text before opening the preview.

Reopening the same cached tab translation reuses its temporary file and focuses that document. If the file was deleted or its saved contents changed, the server creates a fresh file without overwriting it. A newly translated result gets a separate file. Temporary files are still removed on server exit; copies saved with Save As are preserved.

## CodeLens

With `"code_lens": "on"` in Zed's settings, the server shows short commands on a line above text. Zed's default is `"off"`. Zed shows the commands of one line together, separated by `|`.

| Where | Line |
| --- | --- |
| A code comment with no translation | `Translate \| Settings` |
| A translated range | The first line of the translation, or the term and first meaning of a dictionary entry (such as `retain — 保持する`), up to 72 characters, then `Settings` in code |
| A failed range | `Translation failed \| Retry`, then `Settings` in code |
| A range being translated | `Translating… \| Cancel`, then `Settings` in code |
| Above the first line of Markdown and Plain Text | `Translation LSP: Translate in tab \| Auto-translate: On \| <account menu> \| Zed settings` |

`Translate` translates the comment as `Translate selection to …` does, and appears with a provider ready and `selection_action` on. A comment that Hover found already in the target language keeps `Translate`. `Retry` translates a failed range again. After either, the line shows the translation. The message `Translation ready. Hover over the selected text to view it.` shows only if a Code Action waits for the same range.

Translated, failed, and running ranges come from comment Hover, selections, and `Translate`; tab translations get no line. Editing the document removes the lines of the ranges whose translations it invalidates. How a line kept after an edit behaves is described under [CodeLens lines after an edit](behavior.md#codelens-lines-after-an-edit).

In code, a comment with no translation, failure, or translation in progress shows the translation kept for its text, if the server has one. It can come from the same comment at another place or in another document, from before an edit that left the comment as it was, or, with [`disk_cache`](configuration.md#disk-cache), from before a restart.

The translation appears on the comment's line a moment after Zed asks for the CodeLenses. Hover then shows it, and `Delete translation` deletes it, as if the comment had been translated at that place. This needs a provider that is ready and `comment_lens` on. A comment longer than `translation_limits.input_kib.hover` is not looked up, and neither is one found already in the target language, which keeps `Translate`.

Click the first line of a translation to read all of it in a card, or `Translation failed` to read the reason. The card stays until you close it.

- `Retry translating this text` translates the range again, and the CodeLens shows the new translation without a message that it is ready.
- `Delete translation` removes the cached results for that range.
- Zed's copy button copies the text as Markdown, with backslashes before symbols and `---` lines before and after it.

`Translate in tab` translates the whole document as `Translate document to … and open in tab` does. It appears with a provider ready and `document_action` on, in a client that supports `window/showDocument`. The account menu item is the provider's account menu, such as `ChatGPT account…`. `Zed settings` opens Zed's settings file, also only in a client that supports `window/showDocument`.

`Settings` opens a menu with `Turn off automatic translation` or `Turn on automatic translation`, the account menu, and `Open Zed settings file`. It also offers `Delete translations of this document` and `Delete all cached translations` when there is something to delete, as described under [Deleting cached translations](#deleting-cached-translations).

Turning automatic translation off stops comment Hover and automatic Markdown selection from starting translations until the language server restarts. It cancels the translations they started that nothing else asked for. Translations already finished still appear on Hover, and `Translate`, Code Actions, and tab translations keep working. `Auto-translate: On` above Markdown and Plain Text turns it off the same way, and then reads `Auto-translate: Off`.

In a file with many comments, each comment gets a line. With [`features.comment_lens`](configuration.md#features) set to `false`, comments without a translation, a failure, or one in progress get no line, and the menu moves above the first line, as in Markdown.

## Status and notifications

While translation is queued or running, Zed shows a spinner and text directly in the left status bar. `Queued: 1` means one queued translation. `Translating: 1, queued: 0` means one translation is running with none queued. Requests for the same range count as one translation. A tab translation of that range counts separately when it is separate work, as described under [Admission and execution slots](behavior.md#admission-and-execution-slots). Other language-server activity can take priority in this shared area.

The spinner disappears when work ends, including cancellation and timeout. Hover over the original text to check the result. On clients that support server status, translation failures and timeouts leave a warning that clears on retry, deletion of the cached result, editing, or closing the document. Clients without these display capabilities retain Hover and explicit-operation notifications.

Manual completion and failure notifications identify the file name and lines. Automatic completion does not produce a popup. Check error notifications for failed manual translation or management operations.

When the comments of a document cannot be read, the server shows one of the following warnings, once per document until the document is reopened. `<file>` is the file name. See [Documents whose comments cannot be read](languages.md#documents-whose-comments-cannot-be-read).

| Warning | Cause |
| --- | --- |
| `Reading comments in <file> timed out. Comments in this file are not recognized until you reopen it. To allow more time, adjust comment_parse_timeout_ms in the settings.` | The reading took longer than `comment_parse_timeout_ms`. |
| `Reading comments in <file> failed. Comments in this file are not recognized until you reopen it.` | The reading failed inside the server. |

## Canceling and retrying

To stop one accepted translation, place the cursor or a selection in its range and choose `Cancel translation of lines …` from Code Actions. When a tab translation of the range is separate work, as described under [Admission and execution slots](behavior.md#admission-and-execution-slots), it is listed as `Cancel tab translation of lines …`, so one range can offer both actions; canceling one leaves the other running.

`Cancel all translations` stops all translation work without canceling an account menu. Old cancellation actions cannot cancel a later request for the same range. The server also cancels all translations from a progress control, if the client shows one.

To retry a failed range without selecting its text again, place the cursor inside it and choose `Retry translation of lines …` from Code Actions. The action retries the recorded range as an explicit request, including after a usage-limit pause. Hover alone continues to show the failure.

### Deleting cached translations

Use the translation card to delete one range or the `Settings` menu in code to delete a document's results or all cached translations. In Markdown and Plain Text, Code Actions offer these three commands when the provider is ready and no account operation is in progress.

- `Delete translation of lines N-M` when the selection is empty and the cursor is in a translated or failed range. When ranges overlap, the action chooses the newest translation Hover can display, then the newest failure if there is no translation.
- `Delete translations of this document` when the current document has cached results, when translations are kept in memory by what was sent, or when [`disk_cache`](configuration.md#disk-cache) is enabled. It is not offered after the server lost the text of the document to an edit it could not apply, as it could then delete nothing; `Delete all cached translations` still is.
- `Delete all cached translations` when any document has cached results, or in the same two cases. Choose `Delete` in the confirmation card to clear the cache and every kept translation, on disk too, including those of other providers and models. You can keep translating and using menus while the confirmation is open.

What each command removes:

- Deleting one range, with `Delete translation` on its card or `Delete translation of lines N-M`, removes, for exactly that range, the translation for Hover, the tab translation, the recorded failure, and the result that the text is in the target language.
- `Delete translations of this document` removes every result of the document. It also removes the kept translations of what its current text sends with the current settings: the whole text, its parts when a tab sends it in parts, and in code each comment. After a restart, when nothing of the document is in the cache, it still removes these, but not the translations of text you selected earlier; `Delete all cached translations` removes those too.
- `Delete all cached translations` removes the results of every document, including those added while the confirmation was open.

Each command also removes the translations kept by what was sent, described under [Cache](behavior.md#cache), that the deleted results came from. The first two remove them from disk only with `disk_cache` enabled; `Delete all cached translations` removes every kept translation on disk even with `disk_cache` off.

After deletion, the next comment Hover can translate the comment again. To translate a Markdown range again, reselect it or use a manual translation action. Open translation tabs stay open, and work already in progress can add its result to the cache when it finishes.

## What is sent

The extension sends the provider the extracted comment body, without the marks and margin described under [Comment Hover](#comment-hover), the selected text, or the whole document when you explicitly choose document translation. To look up a term, it also sends the line the term is on, at most 200 characters on each side of it.

With every provider except the DeepL API, Google Cloud Translation, and PLaMo Translate, which take no instructions, the model receives the passage as one text, marked as the text to translate. It is instructed to return only its translation, preserving meaning, code, identifiers, and URLs, and to translate any instructions or questions in it instead of following or answering them. These instructions do not guarantee translation accuracy.

The server does not split the translation into JSON fragments or reject it for changes in Markdown formatting or line breaks.

Explicit selection and document actions send the requested text even when it is indented or contains only code. Only automatic background translation checks whether a Markdown selection is prose.

With [`document_split`](configuration.md#document-split) enabled, a translation opened in a tab of a text of more than one and a half times `chunk_kib` that can be divided is sent part by part, each part as a text of its own. For a provider that takes instructions, the instructions of each part add that it is one part of a longer document, that every sentence of it is to be translated, and that no heading, introduction, or sentence is to be added.

The DeepL API and Google Cloud Translation take no instructions. The server sends them the text in segments, with code spans and URLs marked, and puts the translation together again; each part of a text sent in parts is sent the same way. Code blocks are not sent to them, so a text of only code shows as written. See [What is sent](providers/deepl.md#what-is-sent) of the DeepL API page.

PLaMo Translate takes no instructions either, and the server sends it the text as written. The system message holds only the model's settings: the target language, and the source language when `source_language` is a code. Each part of a text sent in parts is sent the same way. See [What is sent](providers/plamo-translate.md#what-is-sent) of the PLaMo Translate page.

To look up a term, the model is instructed to reply with a JSON object of meanings, word parts, and an example. The server builds the entry from that object and escapes every text of the reply, so the reply cannot add links, images, or other formatting to the Hover. A reply that is not such an object is reported as an error; retry it from Code Actions.

The extension does not send conversation history, and does not write translations back into the source document. A text whose translation the server already keeps for the same request is not sent again, except by a retry; see [Cache](behavior.md#cache).

### Text already in the target language

For comment Hover and automatic Markdown selection with `source_language` set to `auto`, a model that takes instructions is also asked for the language of each sentence, where code, identifiers, and URLs do not count. The server shows nothing when every sentence is in the target language. Otherwise it shows the translation, and reports a reply without one as an error.

PLaMo Translate gives no language. For the same translations, the server shows nothing when the reply is the text sent, apart from whitespace at its start and end, whatever `target_language` is. When the model returns such text rewritten, the server shows the reply as a translation. Explicit translations, and every translation with `source_language` set to a language, show the reply.

A model that takes instructions is not asked for the language, and the server always translates, in these cases:

- explicit translations and dictionary lookups
- `source_language` set to a language
- `target_language` set to a language name, such as `Japanese`, to the Chinese code `zh` or `zho`, with or without a region, or to a code that names a script, such as `zh-Hant` or `sr-Latn`

The DeepL API and Google Cloud Translation report the language they detected in each segment, and the server decides from its code the same way, ignoring its region. With a Chinese code or a code that names a script as `target_language`, the server shows nothing only when the detected code is the same in full, ignoring case. Google Cloud Translation takes `zh` and `zh-CN` as the same code.

If you run a Code Action or another explicit translation of a range while an automatic translation of it waits for its reply, and the text is found to be in the target language, the same text is sent again as an explicit translation.

### What is written to disk

With [`disk_cache`](configuration.md#disk-cache) enabled, the server writes each completed translation to an unencrypted file on this computer. The text that was sent and the lines around it are not written. A dictionary entry holds its word and example sentence, and a translation holds what was left untranslated, such as code and identifiers. What the files hold and where they are is described under [Disk cache](configuration.md#disk-cache).

## VS Code

In VS Code, the extension works as described on this page, with these differences:

- Hover titles a translation with a globe icon and `Translation LSP`, without a quote.
- Clicking the first line of a translation, or `Translation failed`, opens the text under that line in place of a card. Its buttons copy the text, translate it again, delete it, and close it. The copy holds the translation as it came, without the characters added to format it, and a dictionary entry as its Markdown. Editing the document, or opening another translation, closes it.
- `Open settings` in the `Settings` menu, and `Settings` above Markdown and Plain Text, open the settings of the extension.
- The line above the text names what it is, such as `Translation to Japanese` or `Translation to Japanese failed`.
- Menus, such as `Settings` and the menu of an API key, open as a list at the top of the window, titled with what they are for. The questions before deleting all cached translations and before removing a saved key open as a dialog with `Cancel`.

The text under a line shows about the first 100,000 characters of a translation; `Copy` holds all of it.

Changed settings apply when the language server restarts. The extension offers `Restart` in a notification. Until you choose it, the running server keeps its settings and its work. `Translation LSP: Restart Language Server` in the Command Palette restarts it at any time.

The extension starts the server for every language in [Languages](languages.md). For languages that VS Code does not support on its own, install an extension for that language so VS Code assigns its language ID. Settings for JavaScript, Kotlin, Proto, and HCL in `features.languages` also apply to the related language IDs that VS Code or those extensions assign, such as `.jsx` and `.kts` files.

The buttons, settings, and messages of the extension follow the display language of VS Code, in English or Japanese. The server's menus, CodeLenses, and messages stay in English.
