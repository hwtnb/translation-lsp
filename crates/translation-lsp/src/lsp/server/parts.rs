//! A translation in a tab sent in parts at once and joined in order.

use std::ops::Range;

use futures_util::{StreamExt, TryStreamExt, stream};
use tokio::task::spawn_blocking;

use crate::{
    config::{
        limits::{SizeLimit, SizeSetting},
        split::DocumentSplit,
    },
    providers::{
        Intent, Kind, Output, Translated, TranslationProvider,
        error::{Cause, Error},
    },
    store::{Content, Key, Sent},
    text::split::parts,
};

use super::worker::{StoreUse, passage};

/// How a translation in a tab is sent in parts.
#[derive(Clone, Copy, Debug)]
pub(super) struct Split {
    pub(super) settings: DocumentSplit,
    /// `translation_limits.output_kib`, which the joined translation is held
    /// to as soon as the parts that completed exceed it.
    pub(super) output: SizeLimit,
    /// Whether the text is Markdown, which is cut between blocks, rather
    /// than plain text, which is cut between paragraphs.
    pub(super) markdown: bool,
}

/// Translates `text` for a tab in the parts that `split` makes, with at most
/// `split.settings.parallel` requests at once, and joins the translations in
/// the order of the text. A part that `store` has is not sent; one that the
/// provider translates is stored as soon as it completes, whatever becomes of
/// the others. The first failure ends the whole translation and drops the
/// requests in progress; no request starts after it, or after a stop of the
/// provider since `generation`. A part whose translation is blank, and a
/// joined translation that would exceed `split.output`, are failures too. A
/// text of one part is sent as it would be without `split`. Returns the
/// translation with the keys of its parts.
pub(super) async fn translate<P: TranslationProvider>(
    provider: &P,
    text: String,
    split: Split,
    target_language: &str,
    context: P::Context,
    generation: u64,
    store: Option<&StoreUse>,
) -> Result<(Translated<P::Context>, Vec<Key>), Error> {
    // A large text takes a while to parse, which would hold up the other
    // translations of the worker.
    let (text, ranges) = spawn_blocking(move || {
        let ranges = parts(&text, split.markdown, split.settings.chunk_bytes());
        (text, ranges)
    })
    .await
    .map_err(|_| Error::failed("Could not divide the document into parts"))?;
    // The provider may have stopped while the text was divided.
    provider.check_admission(&context, generation)?;
    let Some(pieces) = pieces(&text, ranges) else {
        return passage(
            provider,
            Intent::Explicit,
            Output::Document,
            &text,
            target_language,
            (context, generation),
            store,
        )
        .await;
    };
    let keys: Vec<Option<Key>> = pieces
        .iter()
        .map(|piece| {
            store.filter(|_| piece.sent()).map(|used| {
                used.store
                    .key(Output::Document, target_language, Sent::Part(piece.text))
            })
        })
        .collect();
    // What the joined translation holds besides the translations of the
    // parts sent.
    let mut joined_bytes: usize = pieces
        .iter()
        .map(|piece| {
            piece.before.len() + piece.after.len() + if piece.sent() { 0 } else { piece.text.len() }
        })
        .sum();
    let mut translated = Vec::new();
    let mut found = vec![false; pieces.len()];
    if let Some(StoreUse { store, read: true }) = store {
        split.output.check(joined_bytes, SizeSetting::Output)?;
        for (index, key) in keys.iter().enumerate() {
            if let Some(key) = key
                && let Some(Content::Translation(text)) = store.get(*key).await
                && !text.trim().is_empty()
            {
                joined_bytes += text.trim_matches(NEWLINES).len();
                // A text that repeats a part finds it each time.
                split.output.check(joined_bytes, SizeSetting::Output)?;
                found[index] = true;
                translated.push((
                    index,
                    Translated {
                        text,
                        context: context.clone(),
                        kind: Kind::Translation,
                    },
                ));
            }
        }
    }
    let unsent = pieces
        .iter()
        .enumerate()
        .filter(|(index, piece)| piece.sent() && !found[*index]);
    let mut parts = stream::iter(unsent)
        .map(|(index, piece)| {
            let context = context.clone();
            let key = keys[index];
            async move {
                provider.check_admission(&context, generation)?;
                let translated = provider
                    .translate_part(piece.text, target_language, context)
                    .await?;
                // An explicit request always gives a translation.
                if translated.kind != Kind::Translation || translated.text.trim().is_empty() {
                    return Err(Error::failed("A part of the document was not translated"));
                }
                if let (Some(used), Some(key)) = (store, key) {
                    let content = Content::Translation(translated.text.clone());
                    used.store.put(key, content);
                }
                Ok::<_, Error>((index, translated))
            }
        })
        .buffer_unordered(split.settings.parallel);
    while let Some((index, part)) = parts.try_next().await? {
        joined_bytes += part.text.trim_matches(NEWLINES).len();
        split.output.check(joined_bytes, SizeSetting::Output)?;
        translated.push((index, part));
    }
    drop(parts);
    translated.sort_unstable_by_key(|(index, _)| *index);
    let mut contexts = translated.iter().map(|(_, translated)| &translated.context);
    let first = contexts.next().cloned().unwrap_or(context);
    if contexts.any(|other| *other != first) {
        return Err(Cause::Modified("Translation authorization").into());
    }
    let mut translations = translated.into_iter().peekable();
    let mut joined = String::with_capacity(text.len());
    for (index, piece) in pieces.iter().enumerate() {
        joined.push_str(piece.before);
        match translations.next_if(|(at, _)| *at == index) {
            Some((_, translated)) => joined.push_str(translated.text.trim_matches(NEWLINES)),
            None => joined.push_str(piece.text),
        }
        joined.push_str(piece.after);
    }
    let translated = Translated {
        text: joined,
        context: first,
        kind: Kind::Translation,
    };
    Ok((translated, keys.into_iter().flatten().collect()))
}

/// The texts that [`translate`] sends for `text` as parts, or none if it
/// sends the text whole.
pub(super) fn sent_parts(text: &str, markdown: bool, settings: DocumentSplit) -> Vec<&str> {
    pieces(text, parts(text, markdown, settings.chunk_bytes()))
        .into_iter()
        .flatten()
        .filter(Piece::sent)
        .map(|piece| piece.text)
        .collect()
}

/// The pieces of `text` in `ranges`, the parts that [`parts`] makes, or
/// none if the text is sent whole.
fn pieces(text: &str, ranges: Vec<Range<usize>>) -> Option<Vec<Piece<'_>>> {
    (ranges.len() >= 2).then(|| {
        ranges
            .into_iter()
            .map(|range| Piece::of(&text[range]))
            .collect()
    })
}

const NEWLINES: [char; 2] = ['\r', '\n'];

/// A part of the text, with the line breaks around it that it is sent
/// without.
struct Piece<'a> {
    before: &'a str,
    text: &'a str,
    after: &'a str,
}

impl<'a> Piece<'a> {
    fn of(part: &'a str) -> Self {
        let text = part.trim_start_matches(NEWLINES);
        let before = &part[..part.len() - text.len()];
        let trimmed = text.trim_end_matches(NEWLINES);
        Self {
            before,
            after: &text[trimmed.len()..],
            text: trimmed,
        }
    }

    /// Whether the part is sent: one without a letter or a digit, such as a
    /// thematic break alone, is joined as it is.
    fn sent(&self) -> bool {
        self.text.chars().any(char::is_alphanumeric)
    }
}

#[cfg(test)]
mod tests;
