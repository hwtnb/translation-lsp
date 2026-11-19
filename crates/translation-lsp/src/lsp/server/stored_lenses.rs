//! The stored translations of comments that have no result in the cache,
//! such as after a restart with `disk_cache`, after an edit, or when the same
//! comment was translated elsewhere, so that their CodeLenses show them
//! before Hover asks. They are looked up away from the request loop, one
//! lookup per document at a time, and come into the cache as if the provider
//! had given them for their ranges, without a request.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crossbeam_channel::{Receiver, unbounded};

use super::*;
use crate::{
    lsp::{
        document::Comment,
        preview::{Form, markdown::Urls},
    },
    store::{Content, Key, Sent},
};

/// The lookups of stored translations for the CodeLenses of comments.
pub(super) struct Lookups<C> {
    store: Store,
    target_language: String,
    /// `translation_limits.cache.entries`: a lookup finds no more than the
    /// cache keeps.
    limit: usize,
    thread: Thread,
    /// Where the thread sends results, kept until it starts.
    sender: Option<Sender<Looked>>,
    results: Receiver<Looked>,
    /// Ends the lookup in progress when the server stops.
    stop: Arc<AtomicBool>,
    documents: HashMap<Uri, Searches<C>>,
    next_id: u64,
}

/// The thread that looks up, which starts with the first lookup.
enum Thread {
    Unstarted,
    Running {
        jobs: Sender<Lookup>,
        handle: thread::JoinHandle<()>,
    },
    /// It could not start or it ended, and nothing is looked up.
    Stopped,
}

/// The lookups of one open document.
struct Searches<C> {
    /// A state that a lookup ended in with every comment looked up, in
    /// which the document is not looked up again.
    searched: Option<State<C>>,
    /// The ranges that lookups read from the disk, with the version,
    /// session, and context they were read in. In those, the ranges are read
    /// from memory only: the disk gains nothing that this server does not
    /// also put in memory, and what another server writes is found after an
    /// edit or on reopening.
    read: Option<(i32, u64, C, HashSet<Range>)>,
    running: Option<Running<C>>,
    /// Whether CodeLenses were asked for from another state while a lookup
    /// was in progress, so that they are asked for again when it ends.
    again: bool,
}

/// A lookup in progress.
struct Running<C> {
    id: u64,
    state: State<C>,
    /// The comments it looks up.
    ranges: Vec<Range>,
    /// Whether it left out no comment for a translation in progress, so that
    /// its state counts as searched. Such a translation may be canceled
    /// without anything else changing.
    complete: bool,
    /// Ends it when its document is closed or opened again.
    cancel: Arc<AtomicBool>,
}

/// What a lookup's results depend on, other than the cache.
#[derive(Clone, PartialEq)]
struct State<C> {
    version: i32,
    session: u64,
    /// [`Store::changes`].
    changes: u64,
    context: C,
}

/// The comments of a document to look up.
struct Lookup {
    id: u64,
    uri: Uri,
    comments: Vec<Candidate>,
    cancel: Arc<AtomicBool>,
}

struct Candidate {
    range: Range,
    /// The body that a translation of it sends.
    body: String,
    /// Whether it is looked up in memory only.
    memory_only: bool,
}

/// The stored translations that a lookup found.
pub(super) struct Looked {
    id: u64,
    uri: Uri,
    found: Vec<Found>,
}

struct Found {
    range: Range,
    translation: String,
    /// The keys of the comment's translation and of its same-language
    /// result, so that deleting the result forgets both, as it does a
    /// translation from the provider.
    keys: Vec<Key>,
    urls: Urls,
}

impl<C> Lookups<C> {
    /// Lookups of translations of `store` into `target_language`, of at most
    /// `limit` at a time.
    pub(super) fn new(store: Store, target_language: String, limit: usize) -> Self {
        let (sender, results) = unbounded();
        Self {
            store,
            target_language,
            limit,
            thread: Thread::Unstarted,
            sender: Some(sender),
            results,
            stop: Arc::new(AtomicBool::new(false)),
            documents: HashMap::new(),
            next_id: 0,
        }
    }

    /// Where the results of lookups arrive. It is disconnected once the
    /// thread ends or cannot start.
    pub(super) fn results(&self) -> Receiver<Looked> {
        self.results.clone()
    }

    /// Sends `lookup` to the thread, starting it first if needed. Gives
    /// whether it was sent.
    fn send(&mut self, lookup: Lookup) -> bool {
        if matches!(self.thread, Thread::Unstarted) {
            self.thread = match self.spawn() {
                Ok(thread) => thread,
                Err(error) => {
                    eprintln!(
                        "translation-lsp: stored translations cannot be looked up for CodeLenses: {error}"
                    );
                    self.sender = None;
                    Thread::Stopped
                }
            };
        }
        let Thread::Running { jobs, .. } = &self.thread else {
            return false;
        };
        if jobs.send(lookup).is_err() {
            self.stopped();
            return false;
        }
        true
    }

    fn spawn(&mut self) -> ServerResult<Thread> {
        let (jobs, received) = unbounded::<Lookup>();
        let runtime = runtime::Builder::new_current_thread().build()?;
        let store = self.store.clone();
        let target_language = self.target_language.clone();
        let limit = self.limit;
        let stop = self.stop.clone();
        let sender = self.sender.clone().ok_or("no results channel")?;
        let handle = thread::Builder::new()
            .name("stored-lenses".into())
            .spawn(move || {
                for lookup in received {
                    let found =
                        runtime.block_on(look_up(&store, &target_language, &lookup, limit, &stop));
                    let Some(found) = found else {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        continue;
                    };
                    let looked = Looked {
                        id: lookup.id,
                        uri: lookup.uri,
                        found,
                    };
                    if sender.send(looked).is_err() {
                        return;
                    }
                }
            })?;
        // Only the thread sends now, so that the results are disconnected
        // when it ends.
        self.sender = None;
        Ok(Thread::Running { jobs, handle })
    }

    /// Stops looking up after the thread ended unexpectedly.
    pub(super) fn stopped(&mut self) {
        if matches!(self.thread, Thread::Running { .. }) {
            eprintln!(
                "translation-lsp: stored translations are no longer looked up for CodeLenses"
            );
        }
        self.thread = Thread::Stopped;
        self.sender = None;
        self.documents.clear();
    }

    /// Forgets the lookups of a document that was closed or opened again,
    /// ending a lookup in progress.
    pub(super) fn forget(&mut self, uri: &Uri) {
        if let Some(running) = self
            .documents
            .remove(uri)
            .and_then(|searches| searches.running)
        {
            running.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// How many lookups were started, for tests.
    #[cfg(test)]
    pub(super) fn started(&self) -> u64 {
        self.next_id
    }

    /// Whether the thread has started, for tests.
    #[cfg(test)]
    pub(super) fn is_running(&self) -> bool {
        matches!(self.thread, Thread::Running { .. })
    }
}

impl<C> Drop for Lookups<C> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Thread::Running { jobs, handle } =
            std::mem::replace(&mut self.thread, Thread::Stopped)
        {
            drop(jobs);
            let _ = handle.join();
        }
    }
}

/// The stored translations of the comments of `lookup`, up to `limit` of
/// them, or `None` once `stop` or the lookup's cancellation is set.
async fn look_up(
    store: &Store,
    target_language: &str,
    lookup: &Lookup,
    limit: usize,
    stop: &AtomicBool,
) -> Option<Vec<Found>> {
    let mut found = Vec::new();
    for candidate in &lookup.comments {
        if found.len() >= limit {
            break;
        }
        if stop.load(Ordering::Relaxed) || lookup.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let body = candidate.body.as_str();
        let key = store.key(Output::Range, target_language, Sent::Passage(body));
        let content = if candidate.memory_only {
            store.peek(key)
        } else {
            store.get(key).await
        };
        if let Some(Content::Translation(translation)) = content {
            let same_language = store.key(Output::Range, target_language, Sent::SameLanguage(body));
            found.push(Found {
                range: candidate.range,
                translation,
                keys: vec![key, same_language],
                urls: Urls::of(body),
            });
        }
    }
    Some(found)
}

/// Whether a translation of `uri` in progress overlaps `range`.
fn in_progress<C>(scheduler: &Scheduler<C>, uri: &Uri, range: Range) -> bool {
    scheduler
        .translations()
        .values()
        .any(|work| &work.target.uri == uri && overlaps(work.target.range, range))
}

impl<P: TranslationProvider> Server<P> {
    /// Starts a lookup, with `context`, of the stored translations of the
    /// `comments` of `uri` that no result or translation in progress
    /// overlaps, unless the document was looked up in the same state. A
    /// lookup in progress for the document defers it until that one ends.
    pub(super) fn look_up_stored(
        &mut self,
        uri: &Uri,
        context: P::Context,
        comments: Vec<Comment>,
    ) {
        if comments.is_empty() || matches!(self.lookups.thread, Thread::Stopped) {
            return;
        }
        let Some(document) = self.documents.get(uri) else {
            return;
        };
        let state = State {
            version: document.version,
            session: document.session,
            changes: self.store.changes(),
            context,
        };
        if let Some(searches) = self.lookups.documents.get_mut(uri) {
            if searches.searched.as_ref() == Some(&state) {
                return;
            }
            if let Some(running) = &searches.running {
                if running.state != state {
                    searches.again = true;
                }
                return;
            }
        }
        let read = self.lookups.documents.get(uri).and_then(|searches| {
            searches
                .read
                .as_ref()
                .filter(|(version, session, context, _)| {
                    (*version, *session, context) == (state.version, state.session, &state.context)
                })
                .map(|(.., ranges)| ranges)
        });
        // A comment too long for Hover is never translated for its range.
        let limit = self.settings.translation_limits.input_kib.hover.bytes();
        let mut complete = true;
        let mut candidates = Vec::new();
        for comment in comments {
            if comment.length > limit || !self.cache.is_vacant(uri, comment.range) {
                continue;
            }
            if in_progress(&self.scheduler, uri, comment.range) {
                complete = false;
                continue;
            }
            candidates.push(Candidate {
                memory_only: read.is_some_and(|ranges| ranges.contains(&comment.range)),
                range: comment.range,
                body: comment.body,
            });
        }
        if candidates.is_empty() {
            if complete {
                self.searches(uri).searched = Some(state);
            }
            return;
        }
        self.lookups.next_id += 1;
        let id = self.lookups.next_id;
        let cancel = Arc::new(AtomicBool::new(false));
        let ranges = candidates.iter().map(|candidate| candidate.range).collect();
        let lookup = Lookup {
            id,
            uri: uri.clone(),
            comments: candidates,
            cancel: cancel.clone(),
        };
        if !self.lookups.send(lookup) {
            return;
        }
        self.searches(uri).running = Some(Running {
            id,
            state,
            ranges,
            complete,
            cancel,
        });
    }

    fn searches(&mut self, uri: &Uri) -> &mut Searches<P::Context> {
        self.lookups
            .documents
            .entry(uri.clone())
            .or_insert_with(|| Searches {
                searched: None,
                read: None,
                running: None,
                again: false,
            })
    }

    /// Puts the translations that a lookup found into the cache where nothing
    /// took their place meanwhile and the cache has room for them, if the
    /// document is still synchronized and it, the stored translations, and
    /// the context are as the lookup started from. CodeLenses are asked for
    /// again when it adds one, or when the document may need another lookup.
    pub(super) fn take_stored(&mut self, looked: Looked) -> ServerResult<()> {
        let Some(running) = self
            .lookups
            .documents
            .get_mut(&looked.uri)
            .and_then(|searches| searches.running.take_if(|running| running.id == looked.id))
        else {
            return Ok(());
        };
        let state = running.state;
        let current = self.documents.get(&looked.uri).is_some_and(|document| {
            document.version == state.version
                && document.session == state.session
                && document.text().is_some()
        }) && self.store.changes() == state.changes
            && self.provider.context().ok().as_ref() == Some(&state.context);
        let searches = self.searches(&looked.uri);
        let again = std::mem::take(&mut searches.again);
        if current {
            match &mut searches.read {
                Some((version, session, context, ranges))
                    if (*version, *session, &*context)
                        == (state.version, state.session, &state.context) =>
                {
                    ranges.extend(running.ranges);
                }
                read => {
                    *read = Some((
                        state.version,
                        state.session,
                        state.context.clone(),
                        running.ranges.into_iter().collect(),
                    ));
                }
            }
            if running.complete {
                searches.searched = Some(state.clone());
            }
        }
        let mut refresh = again || !current;
        if current {
            for found in looked.found {
                if !self.cache.is_vacant(&looked.uri, found.range)
                    || in_progress(&self.scheduler, &looked.uri, found.range)
                    || !self.cache.has_room(found.translation.len())
                {
                    continue;
                }
                let mut preview = Preview::new(
                    state.context.clone(),
                    found.range,
                    found.translation,
                    true,
                    P::capabilities().hover_note,
                );
                preview.form = Form::Text(found.urls);
                preview.hover = self.settings.hover;
                preview.keys = found.keys;
                self.next_ticket += 1;
                self.cache.insert(
                    looked.uri.clone(),
                    self.next_ticket,
                    Stored::Translation(Output::Range, preview),
                );
                refresh = true;
            }
        }
        if refresh {
            self.ui.refresh_lenses()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Identity;

    #[test]
    fn a_canceled_lookup_reads_nothing_more() {
        let store = Store::new(
            &Settings::default(),
            Identity::new("test"),
            Identity::new("test"),
            None,
        );
        let key = store.key(Output::Range, "ja", Sent::Passage("Alpha."));
        store.put(key, Content::Translation("アルファ。".into()));
        let candidate = || Candidate {
            range: Range::default(),
            body: "Alpha.".into(),
            memory_only: false,
        };
        let mut lookup = Lookup {
            id: 1,
            uri: "file:///a.rs".parse().unwrap(),
            comments: vec![candidate(), candidate()],
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let runtime = runtime::Builder::new_current_thread().build().unwrap();
        let stop = AtomicBool::new(false);
        let found = runtime.block_on(look_up(&store, "ja", &lookup, 64, &stop));
        assert_eq!(found.unwrap().len(), 2);
        let found = runtime.block_on(look_up(&store, "ja", &lookup, 1, &stop));
        assert_eq!(found.unwrap().len(), 1);

        let uses = store.uses();
        lookup.cancel = Arc::new(AtomicBool::new(true));
        assert!(
            runtime
                .block_on(look_up(&store, "ja", &lookup, 64, &stop))
                .is_none()
        );
        assert_eq!(store.uses(), uses);
    }
}
