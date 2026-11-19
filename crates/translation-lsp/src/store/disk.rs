//! Stored translations on disk with `disk_cache`: one file for each key,
//! named by its hash, holding one JSON object.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

use crossbeam_channel::{RecvTimeoutError, Sender, bounded, unbounded};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{Content, Key, Kind};

/// The version of the layout of a file. A file of another version is not
/// read.
const FILE_VERSION: u32 = 1;
/// The size that the files are cut to, the least recently used first.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const CLEAN_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// The age of a temporary file that a write left behind.
const ABANDONED_AFTER: Duration = Duration::from_secs(60 * 60);
/// How often a translation that is used is marked as used on disk.
pub(super) const TOUCH_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Where translations are kept on disk, apart from the API keys.
pub(crate) fn locate() -> Option<PathBuf> {
    dirs::cache_dir().map(|directory| directory.join("translation-lsp").join("translations"))
}

/// The directory of the stored translations.
pub(super) struct Disk {
    directory: PathBuf,
    /// The operations whose failure was told, so that each is told once.
    told: Mutex<HashSet<&'static str>>,
}

#[derive(Deserialize)]
struct Record {
    v: u32,
    kind: String,
    text: Option<String>,
    line: Option<String>,
}

impl Disk {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            told: Mutex::new(HashSet::new()),
        }
    }

    /// Tells the failure of `operation` on stderr, once, without a path or
    /// a text.
    fn tell(&self, operation: &'static str, error: &io::Error) {
        if self
            .told
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(operation)
        {
            eprintln!(
                "translation-lsp: disk cache: could not {operation}: {}",
                error.kind()
            );
        }
    }

    /// What the file of `key` keeps. A file that cannot be read as a
    /// translation of the kind of `key`, or with a text longer than
    /// `output_limit`, is removed.
    pub(super) fn read(&self, key: &Key, output_limit: usize) -> Option<Content> {
        let path = self.directory.join(key.hex());
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
            Err(error) => {
                self.tell("read a stored translation", &error);
                return None;
            }
        };
        if !metadata.is_file() {
            return None;
        }
        // JSON may write each byte of a text in six.
        let most = output_limit.saturating_mul(6).saturating_add(4096) as u64;
        let mut bytes = Vec::new();
        let read =
            fs::File::open(&path).and_then(|file| file.take(most + 1).read_to_end(&mut bytes));
        if let Err(error) = read {
            if error.kind() != io::ErrorKind::NotFound {
                self.tell("read a stored translation", &error);
            }
            return None;
        }
        let content = (bytes.len() as u64 <= most)
            .then(|| parse(&bytes, key.kind(), output_limit))
            .flatten();
        match content {
            Some(content) => {
                if metadata.modified().is_ok_and(|modified| {
                    SystemTime::now()
                        .duration_since(modified)
                        .is_ok_and(|age| age > TOUCH_INTERVAL)
                }) {
                    self.mark_used(&path);
                }
                Some(content)
            }
            None => {
                let _ = fs::remove_file(&path);
                None
            }
        }
    }

    /// Marks the file of `key`, if there is one, as used now.
    fn touch(&self, key: &Key) {
        let path = self.directory.join(key.hex());
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
            self.mark_used(&path);
        }
    }

    fn mark_used(&self, path: &Path) {
        let marked = OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(SystemTime::now()));
        match marked {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                self.tell("mark a stored translation as used", &error);
            }
            _ => (),
        }
    }

    // Lets only the owner use a directory that was there before, such as one
    // made by hand; a new one is made so. Returns whether there is nothing
    // left to do: false while there is no directory.
    #[cfg(unix)]
    fn secure(&self) -> bool {
        use std::os::unix::fs::PermissionsExt;
        match fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700)) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                self.tell("make the stored translations private", &error);
                true
            }
        }
    }

    #[cfg(not(unix))]
    fn secure(&self) -> bool {
        true
    }

    /// Keeps `content` in the file of `key`. It is written to a file of
    /// another name and renamed, so that a reader, in this server or
    /// another, finds the whole of the old file or the new.
    pub(super) fn write(&self, key: &Key, content: &Content) {
        let mut record = json!({"v": FILE_VERSION, "kind": key.kind().name()});
        match content {
            Content::Translation(text) => record["text"] = json!(text),
            Content::SameLanguage => (),
            Content::Entry { text, line } => {
                record["text"] = json!(text);
                record["line"] = json!(line);
            }
        }
        let name = key.hex();
        let result = (|| {
            create_directory(&self.directory)?;
            let temporary = self
                .directory
                .join(format!(".{name}-{}.tmp", Uuid::new_v4()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            let written = file
                .write_all(record.to_string().as_bytes())
                .and_then(|()| {
                    drop(file);
                    fs::rename(&temporary, self.directory.join(&name))
                });
            if written.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            written
        })();
        if let Err(error) = result {
            self.tell("write a stored translation", &error);
        }
    }

    /// Removes the files of `keys`.
    pub(super) fn remove(&self, keys: &[Key]) {
        for key in keys {
            match fs::remove_file(self.directory.join(key.hex())) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    self.tell("remove a stored translation", &error);
                }
                _ => (),
            }
        }
    }

    /// Removes the files unused for longer than `unused_for`, the temporary
    /// files that writes left behind, and then the least recently used files
    /// until the rest take at most `max_bytes`. Files of names that the
    /// server does not write stay.
    pub(super) fn clean(&self, unused_for: Duration, max_bytes: u64, now: SystemTime) {
        let age = |metadata: &fs::Metadata| {
            metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default()
        };
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    self.tell("clean the stored translations", &error);
                }
                return;
            }
        };
        let mut kept = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let stored = is_stored(name);
            if !stored && !is_temporary(name) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let path = entry.path();
            let limit = if stored { unused_for } else { ABANDONED_AFTER };
            if age(&metadata) > limit {
                let _ = fs::remove_file(&path);
            } else if stored {
                kept.push((age(&metadata), metadata.len(), path));
            }
        }
        let mut total: u64 = kept.iter().map(|(_, length, _)| length).sum();
        if total > max_bytes {
            kept.sort_unstable_by_key(|(age, _, _)| std::cmp::Reverse(*age));
            for (_, length, path) in kept {
                if total <= max_bytes {
                    break;
                }
                if fs::remove_file(path).is_ok() {
                    total -= length;
                }
            }
        }
    }

    /// Removes every file of the names that the server writes, and then the
    /// directory if nothing else is in it.
    fn clear(&self) {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    self.tell("delete the stored translations", &error);
                }
                return;
            }
        };
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| is_stored(name) || is_temporary(name))
            {
                match fs::remove_file(entry.path()) {
                    Err(error) if error.kind() != io::ErrorKind::NotFound => {
                        self.tell("delete the stored translations", &error);
                    }
                    _ => (),
                }
            }
        }
        // Fails while another file is in it.
        let _ = fs::remove_dir(&self.directory);
    }
}

/// What the thread of [`Operator`] does to the directory.
pub(super) enum Operation {
    /// Replies with what the file of `key` keeps; see [`Disk::read`].
    Read {
        key: Key,
        output_limit: usize,
        reply: oneshot::Sender<Option<Content>>,
    },
    Write(Key, Content),
    /// Marks the file of a key as used now.
    Touch(Key),
    Remove(Vec<Key>),
    /// Removes every stored translation; see [`Disk::clear`].
    Clear,
}

/// The thread that does the operations on the directory one at a time, in
/// the order they are sent, so that each finds what those before it did.
/// It starts with the first operation. Dropping the operator waits for the
/// operations sent to finish.
pub(super) struct Operator {
    disk: Arc<Disk>,
    state: Mutex<State>,
}

enum State {
    Idle,
    Running {
        sender: Sender<Operation>,
        thread: JoinHandle<()>,
    },
    /// The thread could not start.
    Failed,
}

impl Operator {
    pub(super) fn new(disk: Arc<Disk>) -> Self {
        Self {
            disk,
            state: Mutex::new(State::Idle),
        }
    }

    /// Sends `operation` to the thread, which it starts if none runs.
    /// Returns whether it was sent.
    pub(super) fn send(&self, operation: Operation) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(*state, State::Idle) {
            *state = self.start();
        }
        match &*state {
            State::Running { sender, .. } => sender.send(operation).is_ok(),
            _ => false,
        }
    }

    fn start(&self) -> State {
        let (sender, operations) = unbounded();
        let disk = self.disk.clone();
        let started = thread::Builder::new()
            .name("disk-cache".into())
            .spawn(move || {
                let mut secured = false;
                for operation in operations {
                    if !secured {
                        secured = disk.secure();
                    }
                    match operation {
                        Operation::Read {
                            key,
                            output_limit,
                            reply,
                        } => {
                            let _ = reply.send(disk.read(&key, output_limit));
                        }
                        Operation::Write(key, content) => disk.write(&key, &content),
                        Operation::Touch(key) => disk.touch(&key),
                        Operation::Remove(keys) => disk.remove(&keys),
                        Operation::Clear => disk.clear(),
                    }
                }
            });
        match started {
            Ok(thread) => State::Running { sender, thread },
            Err(error) => {
                eprintln!(
                    "translation-lsp: disk cache: could not start: {}",
                    error.kind()
                );
                State::Failed
            }
        }
    }
}

impl Drop for Operator {
    fn drop(&mut self) {
        let state = std::mem::replace(
            self.state.get_mut().unwrap_or_else(PoisonError::into_inner),
            State::Idle,
        );
        if let State::Running { sender, thread } = state {
            drop(sender);
            let _ = thread.join();
        }
    }
}

/// Cleans the directory now and then every few hours, until the sender it
/// returns is dropped.
pub(super) fn start_cleaner(disk: Arc<Disk>, unused_for: Duration) -> Option<Sender<()>> {
    let (stop, stopped) = bounded::<()>(0);
    let started = thread::Builder::new()
        .name("disk-cache-cleaner".into())
        .spawn(move || {
            loop {
                disk.clean(unused_for, MAX_BYTES, SystemTime::now());
                if stopped.recv_timeout(CLEAN_INTERVAL) != Err(RecvTimeoutError::Timeout) {
                    break;
                }
            }
        });
    match started {
        Ok(_) => Some(stop),
        Err(error) => {
            eprintln!(
                "translation-lsp: disk cache: could not start cleaning: {}",
                error.kind()
            );
            None
        }
    }
}

fn create_directory(directory: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory)
}

/// The content of a file read for a key of `kind`, if it is one.
fn parse(bytes: &[u8], kind: Kind, output_limit: usize) -> Option<Content> {
    let record: Record = serde_json::from_slice(bytes).ok()?;
    if record.v != FILE_VERSION || record.kind != kind.name() {
        return None;
    }
    let content = match (kind, record.text, record.line) {
        (Kind::Passage | Kind::Part, Some(text), None) => Content::Translation(text),
        (Kind::SameLanguage, None, None) => Content::SameLanguage,
        (Kind::Term, Some(text), Some(line)) => Content::Entry { text, line },
        _ => return None,
    };
    (content.text().len() <= output_limit).then_some(content)
}

/// Whether `name` is the name of a stored translation: 64 lowercase
/// hexadecimal digits.
fn is_stored(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether `name` is the name of a file being written.
fn is_temporary(name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(hash, id)| is_stored(hash) && is_uuid(id))
}

fn is_uuid(text: &str) -> bool {
    Uuid::try_parse(text).is_ok_and(|id| id.hyphenated().to_string() == text)
}
