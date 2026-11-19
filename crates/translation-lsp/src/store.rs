//! Completed translations kept by what was sent, wherever it was: in memory
//! while the server runs, and on disk with `disk_cache`. A translation found
//! here is not sent again, whatever document or position asks for it.

pub(crate) mod disk;
pub(crate) mod key;

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

use crossbeam_channel::Sender;
use tokio::sync::oneshot;

use crate::{config::Settings, providers::Output};
use disk::{Disk, Operation, Operator, TOUCH_INTERVAL};
pub(crate) use key::{Identity, Key, Kind, Sent};

/// What a key keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Content {
    /// The translation of a passage or a part.
    Translation(String),
    /// That an automatic request found the text already in the target
    /// language.
    SameLanguage,
    /// A dictionary entry in Markdown, with the line that a CodeLens shows
    /// for it.
    Entry { text: String, line: String },
}

impl Content {
    fn bytes(&self) -> usize {
        match self {
            Self::Translation(text) => text.len(),
            Self::SameLanguage => 0,
            Self::Entry { text, line } => text.len() + line.len(),
        }
    }

    /// The text held to `translation_limits.output_kib`.
    fn text(&self) -> &str {
        match self {
            Self::Translation(text) | Self::Entry { text, .. } => text,
            Self::SameLanguage => "",
        }
    }

    /// Whether a key of `kind` may keep it.
    fn suits(&self, kind: Kind) -> bool {
        matches!(
            (self, kind),
            (Self::Translation(_), Kind::Passage | Kind::Part)
                | (Self::SameLanguage, Kind::SameLanguage)
                | (Self::Entry { .. }, Kind::Term)
        )
    }
}

/// The stored translations of one server, shared by its request loop and its
/// workers.
#[derive(Clone)]
pub(crate) struct Store {
    shared: Arc<Shared>,
}

struct Shared {
    /// What the provider sends for ranges.
    range: Identity,
    /// What the provider sends for tabs.
    document: Identity,
    source_language: String,
    /// `translation_limits.output_kib`: no longer translation is kept.
    output_limit: usize,
    memory: Mutex<Memory>,
    /// The operations on the directory of stored translations, when one can
    /// be found.
    disk: Option<Operator>,
    /// `disk_cache.enabled`. Without it, the directory is only emptied by
    /// `Delete all cached translations`.
    disk_enabled: bool,
    /// Stops the thread that cleans the directory when the store goes away.
    _cleaner: Option<Sender<()>>,
}

impl Store {
    /// `range` and `document` are what the provider sends for each output.
    /// `directory` is where translations are kept on disk with
    /// `disk_cache`; without one, they are kept in memory only.
    pub fn new(
        settings: &Settings,
        range: Identity,
        document: Identity,
        directory: Option<PathBuf>,
    ) -> Self {
        let limits = settings.translation_limits;
        let disk_enabled = settings.disk_cache.enabled && directory.is_some();
        let disk = directory.map(|directory| Arc::new(Disk::new(directory)));
        let cleaner = disk
            .as_ref()
            .filter(|_| disk_enabled)
            .and_then(|disk| disk::start_cleaner(disk.clone(), settings.disk_cache.unused_for()));
        Self {
            shared: Arc::new(Shared {
                range,
                document,
                source_language: settings.source_language.clone(),
                output_limit: limits.output_kib.bytes(),
                memory: Mutex::new(Memory {
                    entries: HashMap::new(),
                    order: BTreeMap::new(),
                    clock: 0,
                    changes: 0,
                    #[cfg(test)]
                    reads: 0,
                    bytes: 0,
                    count_limit: limits.cache.entries,
                    byte_limit: limits
                        .cache
                        .capacity_kib
                        .bytes()
                        .max(limits.output_kib.bytes()),
                }),
                disk: disk.map(Operator::new),
                disk_enabled,
                _cleaner: cleaner,
            }),
        }
    }

    /// The key of `sent` translated into `target_language` for `output`.
    pub fn key(&self, output: Output, target_language: &str, sent: Sent<'_>) -> Key {
        let identity = match output {
            Output::Range => &self.shared.range,
            Output::Document => &self.shared.document,
        };
        Key::new(
            identity,
            target_language,
            &self.shared.source_language,
            sent,
        )
    }

    // Each change to memory sends its operation on the disk before the lock
    // is released, so that the disk changes in the same order.
    fn memory(&self) -> MutexGuard<'_, Memory> {
        self.shared
            .memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The disk, when `disk_cache` uses it.
    fn enabled_disk(&self) -> Option<&Operator> {
        self.shared
            .disk
            .as_ref()
            .filter(|_| self.shared.disk_enabled)
    }

    /// What `key` keeps in memory or, with `disk_cache`, on disk, which then
    /// keeps it in memory too unless memory changed while the disk was read.
    pub async fn get(&self, key: Key) -> Option<Content> {
        let (replied, changes) = {
            let mut memory = self.memory();
            if let Some(content) = self.use_memory(&mut memory, key) {
                return Some(content);
            }
            let disk = self.enabled_disk()?;
            let (reply, replied) = oneshot::channel();
            let read = Operation::Read {
                key,
                output_limit: self.shared.output_limit,
                reply,
            };
            if !disk.send(read) {
                return None;
            }
            #[cfg(test)]
            {
                memory.reads += 1;
            }
            (replied, memory.changes)
        };
        let content = replied.await.ok()??;
        let mut memory = self.memory();
        // What was read may be older than what memory has since been told.
        if memory.changes == changes {
            memory.insert(key, content.clone());
        }
        Some(content)
    }

    /// What `key` keeps in memory, without reading the disk.
    pub fn peek(&self, key: Key) -> Option<Content> {
        self.use_memory(&mut self.memory(), key)
    }

    /// What `key` keeps in memory, marking its file as used now and then.
    fn use_memory(&self, memory: &mut Memory, key: Key) -> Option<Content> {
        let entry = memory.get(&key)?;
        if let Some(disk) = self.enabled_disk()
            && entry.touched.elapsed() >= TOUCH_INTERVAL
        {
            entry.touched = Instant::now();
            disk.send(Operation::Touch(key));
        }
        Some(entry.content.clone())
    }

    /// Keeps `content` for `key` in memory and, with `disk_cache`, on disk,
    /// in place of what the key kept. Content that the key cannot keep, or a
    /// translation longer than `translation_limits.output_kib`, is not kept.
    /// The disk is written later, in another thread.
    pub fn put(&self, key: Key, content: Content) {
        if !content.suits(key.kind()) || content.text().len() > self.shared.output_limit {
            return;
        }
        let mut memory = self.memory();
        memory.changes += 1;
        if let Some(disk) = self.enabled_disk() {
            disk.send(Operation::Write(key, content.clone()));
        }
        memory.insert(key, content);
    }

    /// Forgets the translations of `keys`, in memory and, with
    /// `disk_cache`, on disk, later, in another thread.
    pub fn remove(&self, keys: &[Key]) {
        let mut memory = self.memory();
        memory.changes += 1;
        for key in keys {
            memory.remove(key);
        }
        if let Some(disk) = self.enabled_disk() {
            disk.send(Operation::Remove(keys.to_vec()));
        }
    }

    /// Forgets every translation, in memory and on disk, including those of
    /// other providers and models, whether or not `disk_cache` is enabled.
    /// The disk is emptied later, in another thread.
    pub fn clear(&self) {
        let mut memory = self.memory();
        memory.changes += 1;
        memory.clear();
        if let Some(disk) = &self.shared.disk {
            disk.send(Operation::Clear);
        }
    }

    /// How many times translations were stored or forgotten. Keeping what
    /// was read from disk in memory, or dropping what was used least
    /// recently, is not counted.
    pub fn changes(&self) -> u64 {
        self.memory().changes
    }

    /// Whether memory keeps no translation.
    pub fn is_empty(&self) -> bool {
        self.memory().entries.is_empty()
    }

    /// Whether `disk_cache` keeps translations on disk.
    pub fn disk_enabled(&self) -> bool {
        self.shared.disk_enabled
    }

    /// How many times memory was used or written, for tests that count
    /// lookups.
    #[cfg(test)]
    pub(crate) fn uses(&self) -> u64 {
        self.memory().clock
    }

    /// How many times the disk was read, for tests that count reads.
    #[cfg(test)]
    pub(crate) fn reads(&self) -> u64 {
        self.memory().reads
    }
}

/// The translations in memory, the least recently used dropped first beyond
/// the limits of `translation_limits.cache`.
struct Memory {
    entries: HashMap<Key, Entry>,
    /// The keys by when they were last used.
    order: BTreeMap<u64, Key>,
    clock: u64,
    /// How many times translations were stored or forgotten.
    changes: u64,
    /// How many times the disk was read.
    #[cfg(test)]
    reads: u64,
    bytes: usize,
    count_limit: usize,
    byte_limit: usize,
}

struct Entry {
    content: Content,
    /// When it was last used, by the clock of [`Memory`].
    used: u64,
    /// When the file of the translation was last marked as used, or written.
    touched: Instant,
}

impl Memory {
    fn get(&mut self, key: &Key) -> Option<&mut Entry> {
        let entry = self.entries.get_mut(key)?;
        self.order.remove(&entry.used);
        self.clock += 1;
        entry.used = self.clock;
        self.order.insert(self.clock, *key);
        Some(entry)
    }

    fn insert(&mut self, key: Key, content: Content) {
        self.remove(&key);
        self.clock += 1;
        self.bytes += content.bytes();
        let entry = Entry {
            content,
            used: self.clock,
            touched: Instant::now(),
        };
        self.entries.insert(key, entry);
        self.order.insert(self.clock, key);
        while self.entries.len() > self.count_limit || self.bytes > self.byte_limit {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.bytes -= entry.content.bytes();
            }
        }
    }

    fn remove(&mut self, key: &Key) {
        if let Some(entry) = self.entries.remove(key) {
            self.order.remove(&entry.used);
            self.bytes -= entry.content.bytes();
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests;
