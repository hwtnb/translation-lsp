# Behavior

[日本語](ja/behavior.md)

This page describes what happens to translation work: how much runs at once, what keeps it running, what Hover shows, which translations are kept and reused, and when automatic translation stops. The operations themselves are described under [Usage](usage.md).

## Admission and execution slots

By default, up to two translations run at once, and up to eight are accepted, running or queued, across all open documents. Requests for exactly the same range share one translation. At most one piece of work starts as a tab translation at a time, so range translations keep a slot. Accepted work runs in order, except that range work can pass a tab translation waiting for its slot.

An explicit request refused because the queue is full reports its file and lines and says that it was not accepted. These limits are set under [`translation_limits.work`](configuration.md#work-and-cache-limits).

A tab translation of a range already accepted shares that work. It is separate work, counted and run on its own, in these cases:

- `provider.document` or [`document_split`](configuration.md#document-split) is enabled.
- With the Anthropic API, OpenAI API, and OpenAI-compatible providers, the range request has already been sent.

A tab translation sent in parts is one piece of work with one slot, but it sends up to `document_split.parallel` parts at once, so the provider can receive that many requests from it. It opens only when every part has succeeded: the first part that fails stops the others. After cancellation, expiry, a document edit, a change of account or key, or a stop described under [Pauses](#pauses), no further part is sent.

## What keeps accepted work running

Accepted work keeps running when the selection changes or is cleared, when you switch tabs, and when Zed cancels a Hover response or the response times out. Moving from comment A to comment B keeps A running or queued; return to either comment to read its result. Merely passing over a comment can therefore consume provider usage once Zed sends a Hover request.

Editing the source document, closing it, changing the account or API key, or shutting down the server ends the affected work and removes its translations at those positions. The translations kept by what was sent stay, as described under [Cache](#cache), though shutting down keeps only those on disk with `disk_cache`. Opening and closing a provider's account or key menu leaves translations running.

## Automatic selection

Comment Hover and automatic Markdown selection share a limit of three automatic translations by default. When the limit is reached:

- A comment Hover is not accepted. Return to the comment later, or run `Translate selection to …`.
- Only the latest Markdown selection waits, and the status bar shows `selection not yet accepted`. Selecting D and then E may leave D untranslated.

When a new automatic selection that overlaps the one just before it becomes stable and starts translating, the translation of the earlier selection stops, unless another request also asked for it. Earlier ranges that do not overlap stay translated. One server follows the latest selection, not one selection per editor. A failed background translation is not sent again automatically.

## What Hover returns

Hover first returns the newest completed translation covering the position. A newer pending or failed request never hides a readable completed result. Without a completed result, Hover joins a pending range, or displays its recorded failure with a manual retry instruction. It can find previously requested ranges after deselection, including manual selections in code files.

## Cache

The server keeps completed translations in two ways: by position, for what Hover, CodeLenses, and tabs show, and by what was sent, so that the same text is not sent again.

### Translations at a position

Translations at a position stay in memory within `translation_limits.cache`, counted across all documents; the least recently used go first. Hover shows the newest requested translation that covers its position, and a new translation removes the older ones it fully contains. Each document keeps at most one tab translation.

An explicit translation of exactly the range that a tab translated, or the reverse, reuses that translation when the range is within the input limit of the request that uses it, unless `provider.document` or `document_split` is enabled. A tab translation alone does not appear on Hover.

Editing or closing the document, or changing the account or key, removes the affected translations at positions. After a Markdown range is removed, Hover does not translate it again: select it again or run the Code Action. Comment Hover translates a comment again. Where comment Hover found the text already in the target language, nothing shows until the document changes.

### Translations kept by what was sent

Before sending a text, the server looks for a translation of the same text made with the same provider, model, settings, and languages. One that is found is shown without a request, wherever the text is: at another place, in another document, or after an edit that left the text as it was. These translations stay when the account or the API key changes.

- An automatic request also uses a kept result that the text is in the target language. An explicit request uses only a kept translation, because it always translates.
- Retry actions never use a kept translation: they send the text again, or join the same translation in progress, and replace it.
- A dictionary entry is used again only for the same term on the same line.
- A tab translation sent in parts looks up each part and sends only the parts not found. Each part is kept once it completes, even if the document then fails.
- With the DeepL API and Google Cloud Translation, a selection short enough to be looked up is translated and not kept.

These translations are kept in memory within limits of their own equal to `translation_limits.cache`. With [`disk_cache`](configuration.md#disk-cache), they are also kept on disk and used after a restart. Without it, Hover writes no text to disk; a translation opened in a tab is written to a temporary file, as described under [Opening a translation in a tab](usage.md#opening-a-translation-in-a-tab).

Failed, interrupted, canceled, and expired work never shows a partial translation and is never kept. A failure is shown as an error, not as translated text. What deleting removes is described under [Deleting cached translations](usage.md#deleting-cached-translations).

## CodeLens lines after an edit

Zed keeps a CodeLens line after an edit when it shows the same text, such as the translation of a comment after a line is added above the comment. The line keeps the command of the version it was made for. Clicking it works as follows:

- The line of a translation or of `Translation failed`, and `Retry`, act on their range where it is now. In code, that range must still be a whole comment.
- `Translate` follows where the comment starts and translates the whole comment that starts there now, also after the comment was edited or extended.
- `Translate in tab` translates the whole document as it is now, unless the document was reopened or its synchronization was lost.

These lines, except `Translate in tab`, are refused in these cases:

- An edit touched the range, or, for `Translate`, crossed the start of the comment.
- The comments there now group differently, as when a comment line added right above joins the comment.
- An edit joined a CR and an LF into one line break, as when a CR is typed right before an LF.
- The document was reopened, its text was replaced, or its synchronization was lost.
- The server no longer has every edit made since the line was made, which happens after very many edits.

Code Actions that name a range work only in the version they were made for, as Zed asks for them again each time. After the language server restarts while a file stays open, Zed keeps the lines of the server that ended. They do nothing until the file is closed and opened again; the server cannot change that.

## Deadlines

- A Hover response waits up to `hover_timeout_ms`, 20 seconds by default, from when it arrives, including time in the queue.
- The comment translation behind it keeps running after the response ends, for up to the same time from when the translation starts.
- Selection translations have `selection_timeout_ms`, 30 seconds by default, and tab translations, including a selection opened in a tab, `document_timeout_ms`, 180 seconds, from when they start. Time in the queue comes on top.

The time covers API calls, credential refresh, retries, and streaming. Joining work that is already running never restarts its time; when a selection or tab request with a longer limit joins, the limit grows, counted from the original start. Sign-in and menu operations have limits of their own.

## Automatic retries

An explicit temporary service error, including an overload reported after HTTP 200, is retried at most once within the original deadline.

- The server waits two seconds, or the seconds of a numeric `Retry-After` of up to 120. Any other `Retry-After` leaves the error for you to retry.
- Cancellation, a document change, or a change of account or API key stops a pending retry.
- Connection failures and interrupted streams are not retried automatically.
- A retry sends the same model, `effort`, and `speed`. No failure makes the server send a request again without `speed`.

## Pauses

Usage limits, account restrictions, and configuration errors pause automatic translation and return an error. After fixing the cause, run a selection or document translation explicitly. Automatic translation resumes when that succeeds; restarting or reconnecting is not needed.

Work accepted before the pause sends no new request and keeps its failure, and the pause is not reported again for each of them. The providers that take an API key also pause for billing, key, model, and permission errors; see Errors and pauses on the [Anthropic API](providers/anthropic.md#errors-and-pauses), [OpenAI API](providers/openai-api.md#errors-and-pauses), and [OpenAI-compatible API](providers/openai-compatible.md#errors-and-pauses) pages. When a document is sent in parts, the success of one part counts as a successful explicit translation, even when the document fails as a whole.
