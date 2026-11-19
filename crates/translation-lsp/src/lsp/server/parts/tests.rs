use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use crossbeam_channel::bounded;
use tokio::{
    sync::watch,
    time::{sleep, timeout},
};

use super::*;
use crate::{
    config::limits::TranslationLimits,
    lsp::server::{
        WORKER_EVENT_CHANNEL_CAPACITY,
        worker::{Job, Task, WorkerEvent, run_job},
    },
    providers::{Source, mock::MockContext},
};

/// What the provider of these tests was asked and did.
#[derive(Default)]
struct Record {
    /// Requests for a whole text.
    whole: usize,
    /// The parts sent, in the order they were sent.
    sent: Vec<String>,
    /// The numbers of the parts that completed, in the order they did.
    completed: Vec<usize>,
    running: usize,
    most: usize,
    /// The number of a part that fails.
    fail: Option<usize>,
    /// After this many completed parts, the provider stops admitting work.
    stop_after: Option<usize>,
    /// After this many checks of admission, the provider stops admitting
    /// work.
    stop_after_checks: Option<usize>,
    checks: usize,
    /// The reply to every part, in place of its text in capitals.
    reply: Option<String>,
    /// The number of a part that is answered with a line break alone.
    blank: Option<usize>,
    /// Added to the time each part takes.
    delay: Duration,
}

/// A provider that answers a part with its text in capitals, the later parts
/// first.
#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Record>>);

/// Counts a part as running until it completes or is dropped.
struct Running(Arc<Mutex<Record>>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.lock().unwrap().running -= 1;
    }
}

/// The number in the heading `# Part N` that `text` starts with.
fn number(text: &str) -> usize {
    text.strip_prefix("# Part ")
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_default()
}

impl Recorder {
    fn record(&self) -> MutexGuard<'_, Record> {
        self.0.lock().unwrap()
    }
}

impl TranslationProvider for Recorder {
    type Context = MockContext;

    fn context(&self) -> Result<MockContext, Error> {
        Ok(MockContext)
    }

    fn check_admission(&self, _context: &MockContext, _generation: u64) -> Result<(), Error> {
        let mut record = self.record();
        record.checks += 1;
        let stopped = record
            .stop_after
            .is_some_and(|count| record.completed.len() >= count)
            || record
                .stop_after_checks
                .is_some_and(|count| record.checks > count);
        if stopped {
            Err(Error::failed("Stopped"))
        } else {
            Ok(())
        }
    }

    async fn translate(
        &self,
        _intent: Intent,
        _output: Output,
        text: &str,
        _target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, Error> {
        self.record().whole += 1;
        Ok(Translated {
            text: text.to_uppercase(),
            context,
            kind: Kind::Translation,
        })
    }

    async fn translate_part(
        &self,
        text: &str,
        _target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, Error> {
        let part = number(text);
        let delay = {
            let mut record = self.record();
            record.sent.push(text.to_owned());
            record.running += 1;
            record.most = record.most.max(record.running);
            record.delay
        };
        let _running = Running(self.0.clone());
        sleep(delay + Duration::from_millis(20 * 10_u64.saturating_sub(part as u64))).await;
        let mut record = self.record();
        if record.fail == Some(part) {
            return Err(Error::failed("Synthetic part failure"));
        }
        record.completed.push(part);
        let text = if record.blank == Some(part) {
            "\r\n".to_owned()
        } else {
            record.reply.clone().unwrap_or_else(|| text.to_uppercase())
        };
        Ok(Translated {
            text: format!("\n{text}\n\n"),
            context,
            kind: Kind::Translation,
        })
    }
}

/// A Markdown document of `count` sections of about 600 bytes, each its own
/// part when parts aim at 1 KiB.
fn document(count: usize) -> String {
    (1..=count)
        .map(|part| format!("# Part {part}\n\n{}\n\n", "word ".repeat(115).trim_end()))
        .collect()
}

fn split(parallel: usize) -> Split {
    Split {
        settings: DocumentSplit {
            enabled: true,
            parallel,
            chunk_kib: 1,
        },
        output: TranslationLimits::default().output_kib,
        markdown: true,
    }
}

async fn run(
    provider: &Recorder,
    text: &str,
    split: Split,
) -> Result<Translated<MockContext>, Error> {
    translate(provider, text.into(), split, "ja", MockContext, 0).await
}

#[tokio::test(flavor = "current_thread")]
async fn parts_are_joined_in_the_order_of_the_text() {
    let text = document(6);
    let provider = Recorder::default();
    let translated = run(&provider, &text, split(8)).await.unwrap();
    assert_eq!(translated.text, text.to_uppercase());
    let record = provider.record();
    assert_eq!(record.whole, 0);
    assert_eq!(record.sent.len(), 6);
    // The later parts completed first.
    assert_eq!(record.completed, [6, 5, 4, 3, 2, 1]);
    // A part is sent without the line breaks around it.
    assert!(
        record
            .sent
            .iter()
            .all(|sent| sent.trim_matches(['\r', '\n']) == sent)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn line_breaks_between_parts_stay_as_written_and_blank_parts_are_not_sent() {
    let text = format!(
        "{}{}",
        "\r\n".repeat(1500),
        document(3).replace('\n', "\r\n")
    );
    let provider = Recorder::default();
    let translated = run(&provider, &text, split(4)).await.unwrap();
    assert_eq!(translated.text, text.to_uppercase());
    assert_eq!(parts(&text, true, 1024).len(), 4);
    assert_eq!(provider.record().sent.len(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn no_more_than_parallel_parts_are_sent_at_once() {
    let text = document(6);
    for parallel in [1, 2, 4] {
        let provider = Recorder::default();
        run(&provider, &text, split(parallel)).await.unwrap();
        let record = provider.record();
        assert_eq!(record.most, parallel);
        assert_eq!(record.sent.len(), 6);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn one_failed_part_fails_the_translation_and_sends_no_more() {
    let text = document(6);
    // One at a time: the parts after the failed one are never sent.
    let provider = Recorder::default();
    provider.record().fail = Some(2);
    let error = run(&provider, &text, split(1)).await.err().unwrap();
    assert_eq!(error.to_string(), "Synthetic part failure");
    assert_eq!(provider.record().sent.len(), 2);
    // Two at once: part 2 completes, part 3 is sent, and part 1 fails while
    // part 3 is in progress, which drops it.
    let provider = Recorder::default();
    provider.record().fail = Some(1);
    assert!(run(&provider, &text, split(2)).await.is_err());
    sleep(Duration::from_millis(300)).await;
    let record = provider.record();
    assert_eq!(record.sent.len(), 3);
    assert_eq!(record.completed, [2]);
    assert_eq!(record.running, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stop_of_the_provider_sends_no_more_parts() {
    let provider = Recorder::default();
    provider.record().stop_after = Some(2);
    let error = run(&provider, &document(6), split(1)).await.err().unwrap();
    assert_eq!(error.to_string(), "Stopped");
    assert_eq!(provider.record().sent.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_and_the_deadline_send_no_more_parts() {
    for cancel in [true, false] {
        let provider = Recorder::default();
        provider.record().delay = Duration::from_millis(200);
        let (stop, deadline) = watch::channel(Some(Instant::now() + Duration::from_secs(30)));
        let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
        let job = Job {
            ticket: 1,
            deadline,
            task: Task::Translation {
                intent: Intent::Explicit,
                output: Output::Document,
                generation: 0,
                source: Source::Passage(document(6)),
                target_language: "ja".into(),
                context: MockContext,
                split: Some(split(2)),
            },
        };
        let running = run_job(job, &finished, &provider);
        tokio::pin!(running);
        let sent = async {
            while provider.record().sent.len() < 2 {
                sleep(Duration::from_millis(5)).await;
            }
        };
        tokio::select! {
            () = &mut running => panic!("The translation completed"),
            () = sent => (),
        }
        stop.send_replace(if cancel { None } else { Some(Instant::now()) });
        timeout(Duration::from_secs(1), &mut running).await.unwrap();
        assert!(matches!(
            events.try_recv().unwrap(),
            WorkerEvent::Started(1)
        ));
        let WorkerEvent::Finished(completion) = events.try_recv().unwrap() else {
            panic!("Expected the job to finish");
        };
        let expected = if cancel {
            "Translation canceled"
        } else {
            "Translation timed out"
        };
        assert_eq!(completion.error().unwrap().to_string(), expected);
        sleep(Duration::from_millis(500)).await;
        let record = provider.record();
        assert_eq!(record.sent.len(), 2, "{expected}");
        assert!(record.completed.is_empty());
        assert_eq!(record.running, 0);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_text_is_sent_whole_without_split_or_when_it_is_small() {
    let provider = Recorder::default();
    let small = document(2);
    assert!(small.len() <= 1536);
    let translated = run(&provider, &small, split(4)).await.unwrap();
    assert_eq!(translated.text, small.to_uppercase());
    // Through the worker, without `split`.
    let (_stop, deadline) = watch::channel(Some(Instant::now() + Duration::from_secs(30)));
    let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
    let job = Job {
        ticket: 1,
        deadline,
        task: Task::Translation {
            intent: Intent::Explicit,
            output: Output::Document,
            generation: 0,
            source: Source::Passage(document(6)),
            target_language: "ja".into(),
            context: MockContext,
            split: None,
        },
    };
    run_job(job, &finished, &provider).await;
    events.try_recv().unwrap();
    assert!(matches!(events.try_recv().unwrap(), WorkerEvent::Finished(f) if f.is_ok()));
    let record = provider.record();
    assert_eq!(record.whole, 2);
    assert!(record.sent.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn a_blank_translation_of_a_part_fails_the_translation_and_sends_no_more() {
    let provider = Recorder::default();
    provider.record().blank = Some(2);
    let error = run(&provider, &document(3), split(1)).await.err().unwrap();
    assert_eq!(
        error.to_string(),
        "A part of the document was not translated"
    );
    let record = provider.record();
    assert_eq!(record.sent.len(), 2);
    assert_eq!(record.completed, [1, 2]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stop_while_the_text_is_divided_sends_nothing() {
    for text in ["A paragraph.\n\nAnother.".to_owned(), document(4)] {
        let provider = Recorder::default();
        provider.record().stop_after_checks = Some(1);
        let (_stop, deadline) = watch::channel(Some(Instant::now() + Duration::from_secs(30)));
        let (finished, events) = bounded(WORKER_EVENT_CHANNEL_CAPACITY);
        let job = Job {
            ticket: 1,
            deadline,
            task: Task::Translation {
                intent: Intent::Explicit,
                output: Output::Document,
                generation: 0,
                source: Source::Passage(text),
                target_language: "ja".into(),
                context: MockContext,
                split: Some(split(4)),
            },
        };
        run_job(job, &finished, &provider).await;
        events.try_recv().unwrap();
        let WorkerEvent::Finished(completion) = events.try_recv().unwrap() else {
            panic!("Expected the job to finish");
        };
        assert_eq!(completion.error().unwrap().to_string(), "Stopped");
        let record = provider.record();
        // The worker checked before the text was divided, and again after.
        assert_eq!(record.checks, 2);
        assert_eq!((record.whole, record.sent.len()), (0, 0));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_joined_translation_over_the_output_limit_sends_no_more_parts() {
    let provider = Recorder::default();
    provider.record().reply = Some("x".repeat(800));
    let mut limited = split(1);
    limited.output = SizeLimit::try_from(1).unwrap();
    let error = run(&provider, &document(6), limited).await.err().unwrap();
    let message = error.to_string();
    assert!(message.starts_with("Translation size "), "{message}");
    assert!(
        message.ends_with("exceeds 1 KiB; adjust translation_limits.output_kib"),
        "{message}"
    );
    assert_eq!(provider.record().sent.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn a_part_without_letters_or_digits_is_joined_as_it_is() {
    // Front matter begins with a thematic break, and the setext heading
    // after it starts a long section.
    let mut text = "---\ntitle: Guide\nauthor: someone\n---\n\n".to_owned();
    for index in 0..8 {
        text.push_str(&format!(
            "Intro {index} {}\n\n",
            "word ".repeat(58).trim_end()
        ));
    }
    text.push_str(&document(1));
    assert_eq!(&text[parts(&text, true, 1024)[0].clone()], "---\n");
    let provider = Recorder::default();
    let translated = run(&provider, &text, split(4)).await.unwrap();
    assert_eq!(translated.text, text.to_uppercase());
    assert!(translated.text.starts_with("---\nTITLE: GUIDE"));
    let record = provider.record();
    assert!(
        record.sent.iter().all(|sent| sent != "---"),
        "{:?}",
        record.sent
    );
    assert_eq!(record.sent.len(), parts(&text, true, 1024).len() - 1);
}
