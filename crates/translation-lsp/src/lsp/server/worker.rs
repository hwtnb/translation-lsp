use std::future::Future;

use tokio::time::sleep_until;

use crate::{
    providers::{
        Intent, Kind, Source,
        unset::{NO_SETTINGS_FILE, settings_file},
    },
    store::{Content, Key, Sent, Store},
};

use super::{
    parts::{self, Split},
    *,
};

const ADMIN_CONCURRENCY: usize = 1;

pub(super) struct Job<C> {
    pub(super) ticket: u64,
    pub(super) deadline: watch::Receiver<Option<Instant>>,
    pub(super) task: Task<C>,
}

pub(super) enum Task<C> {
    AccountMenu {
        ui: Ui,
    },
    /// The server menu and the commands it offers.
    Menu {
        ui: Ui,
        message: String,
        choices: Vec<Command>,
    },
    /// Opens Zed's settings file in the editor.
    OpenSettings {
        ui: Ui,
    },
    Translation {
        intent: Intent,
        output: Output,
        generation: u64,
        source: Source,
        target_language: String,
        context: C,
        /// How a translation for a tab is sent in parts, if it is.
        split: Option<Split>,
        /// The stored translations it uses, if any.
        store: Option<StoreUse>,
    },
    Administration {
        command: String,
        ui: Ui,
    },
}

/// How a translation uses the stored translations.
pub(super) struct StoreUse {
    pub(super) store: Store,
    /// Whether it looks for a stored translation before sending. A retry
    /// does not: it sends, and its result takes the place of what was stored.
    pub(super) read: bool,
}

/// A job that ended, with the result of its kind of work.
pub(super) enum Finished<C> {
    /// A translation, with the keys its result is stored under, which only
    /// a success has.
    Translation(Completion<Translated<C>>, Vec<Key>),
    Administration(Completion<Administered>),
}

pub(super) struct Completion<T> {
    pub(super) ticket: u64,
    pub(super) result: Result<T, ProviderError>,
}

/// What an administration job gives.
pub(super) enum Administered {
    /// The command chosen from the account menu or another menu, or none.
    Choice(Option<Command>),
    /// An operation that has nothing to tell.
    Done,
    /// A message for the user.
    Message(String),
}

impl<C> Finished<C> {
    pub(super) fn ticket(&self) -> u64 {
        match self {
            Self::Translation(completion, _) => completion.ticket,
            Self::Administration(completion) => completion.ticket,
        }
    }

    #[cfg(test)]
    pub(super) fn error(&self) -> Option<&ProviderError> {
        match self {
            Self::Translation(completion, _) => completion.result.as_ref().err(),
            Self::Administration(completion) => completion.result.as_ref().err(),
        }
    }

    #[cfg(test)]
    pub(super) fn is_ok(&self) -> bool {
        self.error().is_none()
    }
}

pub(super) enum WorkerEvent<C> {
    Started(u64),
    Finished(Finished<C>),
}

pub(super) async fn worker<P: TranslationProvider>(
    jobs: JobReceiver<P::Context>,
    administrations: JobReceiver<P::Context>,
    finished: Sender<WorkerEvent<P::Context>>,
    provider: P,
    concurrency: usize,
) {
    tokio::join!(
        run_jobs(
            administrations,
            finished.clone(),
            provider.clone(),
            ADMIN_CONCURRENCY,
        ),
        run_jobs(jobs, finished, provider, concurrency),
    );
}

async fn run_jobs<P: TranslationProvider>(
    jobs: JobReceiver<P::Context>,
    finished: Sender<WorkerEvent<P::Context>>,
    provider: P,
    concurrency: usize,
) {
    stream::unfold(jobs, |mut jobs| async move {
        jobs.recv().await.map(|job| (job, jobs))
    })
    .for_each_concurrent(concurrency, |job| run_job(job, &finished, &provider))
    .await;
}

pub(super) async fn run_job<P: TranslationProvider>(
    job: Job<P::Context>,
    finished: &Sender<WorkerEvent<P::Context>>,
    provider: &P,
) {
    let Job {
        ticket,
        mut deadline,
        task,
    } = job;
    let event = match task {
        Task::Translation {
            intent,
            output,
            generation,
            source,
            target_language,
            context,
            split,
            store,
        } => {
            let started = || finished.send(WorkerEvent::Started(ticket)).is_ok();
            let result = until_deadline(&mut deadline, started, async {
                provider.check_admission(&context, generation)?;
                match source {
                    Source::Passage(text) => match split {
                        Some(split) if intent == Intent::Explicit && output == Output::Document => {
                            parts::translate(
                                provider,
                                text,
                                split,
                                &target_language,
                                context.clone(),
                                generation,
                                store.as_ref(),
                            )
                            .await
                        }
                        _ => {
                            passage(
                                provider,
                                intent,
                                output,
                                &text,
                                &target_language,
                                (context.clone(), generation),
                                store.as_ref(),
                            )
                            .await
                        }
                    },
                    Source::Term { term, line } => {
                        look_up(
                            provider,
                            intent,
                            output,
                            (&term, &line),
                            &target_language,
                            (context.clone(), generation),
                            store.as_ref(),
                        )
                        .await
                    }
                }
            })
            .await;
            let (result, keys) = match result {
                Ok((translated, keys)) => (Ok(translated), keys),
                Err(error) => (Err(error), Vec::new()),
            };
            Finished::Translation(Completion { ticket, result }, keys)
        }
        Task::AccountMenu { ui } => {
            let result = until_deadline(&mut deadline, || true, async {
                let commands = provider.account_commands();
                let titles: Vec<_> = commands
                    .iter()
                    .map(|command| command.title.to_owned())
                    .collect();
                ui.choose("", &titles).await.map(|choice| {
                    Administered::Choice(choice.and_then(|title| {
                        commands
                            .into_iter()
                            .find(|command| command.title == title)
                            .map(|command| Command::new(title, command.command.into(), None))
                    }))
                })
            })
            .await;
            Finished::Administration(Completion { ticket, result })
        }
        Task::Menu {
            ui,
            message,
            choices,
        } => {
            let result = until_deadline(&mut deadline, || true, async {
                let titles: Vec<_> = choices
                    .iter()
                    .map(|command| command.title.clone())
                    .collect();
                ui.choose(&message, &titles).await.map(|choice| {
                    Administered::Choice(
                        choice.and_then(|title| choices.into_iter().find(|c| c.title == title)),
                    )
                })
            })
            .await;
            Finished::Administration(Completion { ticket, result })
        }
        Task::OpenSettings { ui } => {
            let result = until_deadline(&mut deadline, || true, async {
                let unopened = || ProviderError::failed(NO_SETTINGS_FILE);
                let path = settings_file().ok_or_else(unopened)?;
                ui.edit(&path).await.map_err(|_| unopened())?;
                Ok(Administered::Done)
            })
            .await;
            Finished::Administration(Completion { ticket, result })
        }
        Task::Administration { command, ui } => {
            let result = until_deadline(&mut deadline, || true, async {
                provider
                    .administer(&command, &ui)
                    .await
                    .map(Administered::Message)
            })
            .await;
            Finished::Administration(Completion { ticket, result })
        }
    };
    // Every dispatched job returns its slot, including cancellation before start.
    let _ = finished.send(WorkerEvent::Finished(event));
}

/// Translates a whole text, unless `store` has it: with an automatic
/// request, a translation or that the text is already in the target
/// language; with an explicit one, a translation only, as explicit requests
/// always translate. A result from the provider is stored. Returns the
/// result with the keys of both, so that deleting the result forgets the
/// other too.
pub(super) async fn passage<P: TranslationProvider>(
    provider: &P,
    intent: Intent,
    output: Output,
    text: &str,
    target_language: &str,
    (context, generation): (P::Context, u64),
    store: Option<&StoreUse>,
) -> Result<(Translated<P::Context>, Vec<Key>), ProviderError> {
    let Some(StoreUse { store, read }) = store else {
        return provider
            .translate(intent, output, text, target_language, context)
            .await
            .map(|translated| (translated, Vec::new()));
    };
    let translation = store.key(output, target_language, Sent::Passage(text));
    let same_language = store.key(output, target_language, Sent::SameLanguage(text));
    let keys = vec![translation, same_language];
    if *read {
        if let Some(Content::Translation(text)) = store.get(translation).await {
            let translated = Translated {
                text,
                context,
                kind: Kind::Translation,
            };
            return Ok((translated, keys));
        }
        if intent == Intent::Automatic
            && store.get(same_language).await == Some(Content::SameLanguage)
        {
            let translated = Translated {
                text: String::new(),
                context,
                kind: Kind::SameLanguage,
            };
            return Ok((translated, keys));
        }
        // The provider may have stopped while the disk was read.
        provider.check_admission(&context, generation)?;
    }
    let translated = provider
        .translate(intent, output, text, target_language, context)
        .await?;
    match translated.kind {
        Kind::Translation => store.put(translation, Content::Translation(translated.text.clone())),
        Kind::SameLanguage => store.put(same_language, Content::SameLanguage),
        Kind::Entry(_) => (),
    }
    Ok((translated, keys))
}

/// Looks up `term`, selected from `line`, unless `store` has its entry, and
/// stores an entry from the provider. A provider without entries translates
/// the term, which is not stored.
async fn look_up<P: TranslationProvider>(
    provider: &P,
    intent: Intent,
    output: Output,
    (term, line): (&str, &str),
    target_language: &str,
    (context, generation): (P::Context, u64),
    store: Option<&StoreUse>,
) -> Result<(Translated<P::Context>, Vec<Key>), ProviderError> {
    let key = store.map(|used| {
        used.store
            .key(output, target_language, Sent::Term { term, line })
    });
    if let (Some(StoreUse { store, read: true }), Some(key)) = (store, key) {
        if let Some(Content::Entry { text, line }) = store.get(key).await {
            let translated = Translated {
                text,
                context,
                kind: Kind::Entry(line),
            };
            return Ok((translated, vec![key]));
        }
        // The provider may have stopped while the disk was read.
        provider.check_admission(&context, generation)?;
    }
    let translated = provider
        .look_up(intent, output, term, line, target_language, context)
        .await?;
    let mut keys = Vec::new();
    if let (Some(used), Some(key), Kind::Entry(line)) = (store, key, &translated.kind) {
        let content = Content::Entry {
            text: translated.text.clone(),
            line: line.clone(),
        };
        used.store.put(key, content);
        keys.push(key);
    }
    Ok((translated, keys))
}

/// Runs `operation` until it completes or its deadline passes. `start` runs
/// once the deadline is known to be ahead; `false` from it cancels the job.
async fn until_deadline<T>(
    deadline: &mut watch::Receiver<Option<Instant>>,
    start: impl FnOnce() -> bool,
    operation: impl Future<Output = Result<T, ProviderError>>,
) -> Result<T, ProviderError> {
    let Some(current) = *deadline.borrow_and_update() else {
        return Err(ProviderError::cancelled());
    };
    if current <= Instant::now() || !start() {
        return Err(ProviderError::cancelled());
    }
    // Keep the same operation alive when a waiter extends or shortens its deadline.
    tokio::pin!(operation);
    let timer = sleep_until(current.into());
    tokio::pin!(timer);
    loop {
        tokio::select! {
            biased;
            changed = deadline.changed() => {
                if changed.is_err() {
                    return Err(ProviderError::cancelled());
                }
                let Some(current) = *deadline.borrow_and_update() else {
                    return Err(ProviderError::cancelled());
                };
                timer.as_mut().reset(current.into());
            }
            _ = &mut timer => break Err(ProviderError::failed("Translation timed out")),
            result = &mut operation => break result,
        }
    }
}
