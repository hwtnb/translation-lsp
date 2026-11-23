use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::{
    config::{AnthropicEffort, AnthropicModel, AnthropicSpeed, CompatibleModel},
    providers::{
        OutputOptions, TranslationProvider, anthropic::AnthropicProvider,
        openai_compatible::CompatibleProvider, plamo_translate::PlamoProvider,
    },
};

/// A directory of a test's own, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = env::temp_dir().join(format!("translation-lsp-store-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The directory of the stored translations, which the store creates.
    fn translations(&self) -> PathBuf {
        self.0.join("translations")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn settings(options: serde_json::Value) -> Settings {
    Settings::from_options(Some(options)).unwrap()
}

fn store(options: serde_json::Value, directory: Option<&Path>) -> Store {
    Store::new(
        &settings(options),
        Identity::new("mock"),
        Identity::new("mock"),
        directory.map(Path::to_owned),
    )
}

fn passage(store: &Store, text: &str) -> Key {
    store.key(Output::Range, "ja", Sent::Passage(text))
}

fn translation(text: &str) -> Content {
    Content::Translation(text.into())
}

fn anthropic(model: AnthropicModel, directory: &Path) -> Identity {
    AnthropicProvider::synthetic("http://127.0.0.1:9".into(), directory)
        .with_options(OutputOptions::new(model, None))
        .identity(Output::Range)
}

fn haiku() -> AnthropicModel {
    AnthropicModel {
        model: "claude-haiku-5-5".into(),
        effort: AnthropicEffort::Low,
        thinking: None,
        speed: None,
    }
}

fn compatible(base_url: &str, model: &str, effort: Option<&str>) -> Identity {
    let scratch = Scratch::new();
    CompatibleProvider::new(
        base_url,
        OutputOptions::new(
            CompatibleModel {
                model: model.into(),
                effort: effort.map(Into::into),
            },
            None,
        ),
        "auto".into(),
        Settings::default().translation_limits.output_kib,
        Ok(scratch.0.clone()),
    )
    .unwrap()
    .identity(Output::Range)
}

#[test]
fn a_key_changes_with_each_field_and_has_a_fixed_form() {
    let identity = Identity::new("anthropic_api")
        .with("model", "claude-haiku-5-5")
        .with("effort", AnthropicEffort::Low)
        .with("thinking", false);
    let key = |identity: &Identity, target, source, sent| Key::new(identity, target, source, sent);
    let base = key(&identity, "ja", "auto", Sent::Passage("Retry."));
    // Written by an implementation of its own: SHA-256 of each name and value
    // after its length as eight little-endian bytes.
    assert_eq!(
        base.hex(),
        "41a92efb8183356e509a26a9cec60f92a17248480669f73176bfb2e321bb0efc"
    );
    assert_eq!(base, key(&identity, "ja", "auto", Sent::Passage("Retry.")));
    let others = [
        key(
            &Identity::new("openai_api")
                .with("model", "claude-haiku-5-5")
                .with("effort", AnthropicEffort::Low)
                .with("thinking", false),
            "ja",
            "auto",
            Sent::Passage("Retry."),
        ),
        key(
            &Identity::new("anthropic_api")
                .with("model", "claude-sonnet-5-5")
                .with("effort", AnthropicEffort::Low)
                .with("thinking", false),
            "ja",
            "auto",
            Sent::Passage("Retry."),
        ),
        key(
            &Identity::new("anthropic_api")
                .with("model", "claude-haiku-5-5")
                .with("effort", AnthropicEffort::High)
                .with("thinking", false),
            "ja",
            "auto",
            Sent::Passage("Retry."),
        ),
        key(&identity, "en", "auto", Sent::Passage("Retry.")),
        key(&identity, "ja", "en", Sent::Passage("Retry.")),
        key(&identity, "ja", "auto", Sent::Part("Retry.")),
        key(&identity, "ja", "auto", Sent::SameLanguage("Retry.")),
        key(&identity, "ja", "auto", Sent::Passage("Retry")),
        key(
            &identity,
            "ja",
            "auto",
            Sent::Term {
                term: "Retry.",
                line: "",
            },
        ),
        key(
            &identity,
            "ja",
            "auto",
            Sent::Term {
                term: "Retry",
                line: "Retry.",
            },
        ),
    ];
    for (index, other) in others.iter().enumerate() {
        assert_ne!(*other, base, "{index}");
        assert!(others[index + 1..].iter().all(|later| later != other));
    }
    // Lengths keep one field from running into the next.
    assert_ne!(
        key(&Identity::new("ab"), "c", "", Sent::Passage("")),
        key(&Identity::new("a"), "bc", "", Sent::Passage("")),
    );
}

#[test]
fn provider_identities_leave_out_speed_and_the_key_but_not_the_service() {
    let first = Scratch::new();
    let second = Scratch::new();
    let fast = AnthropicModel {
        speed: Some(AnthropicSpeed::Fast),
        ..haiku()
    };
    assert_eq!(anthropic(haiku(), &first.0), anthropic(fast, &second.0));
    let effort = AnthropicModel {
        effort: AnthropicEffort::High,
        ..haiku()
    };
    assert_ne!(anthropic(haiku(), &first.0), anthropic(effort, &first.0));
    let model = AnthropicModel {
        model: "claude-sonnet-5-5".into(),
        ..haiku()
    };
    assert_ne!(anthropic(haiku(), &first.0), anthropic(model, &first.0));
    // What is sent for thinking, not how the setting is written.
    let thinking = AnthropicModel {
        thinking: Some(false),
        ..haiku()
    };
    assert_eq!(anthropic(haiku(), &first.0), anthropic(thinking, &first.0));
    let thinking = AnthropicModel {
        thinking: Some(true),
        ..haiku()
    };
    assert_ne!(anthropic(haiku(), &first.0), anthropic(thinking, &first.0));

    let service = compatible("http://127.0.0.1:8080/v1", "local", None);
    assert_eq!(
        service,
        compatible("http://127.0.0.1:8080/v1/", "local", None)
    );
    for other in [
        compatible("http://127.0.0.1:8081/v1", "local", None),
        compatible("http://127.0.0.1:8080/v1", "other", None),
        compatible("http://127.0.0.1:8080/v1", "local", Some("low")),
    ] {
        assert_ne!(service, other);
    }
}

#[test]
fn plamo_translate_keeps_its_translations_apart_from_the_same_compatible_service() {
    let scratch = Scratch::new();
    let plamo = |base_url: &str, model: &str| {
        PlamoProvider::new(
            base_url,
            model.into(),
            "auto".into(),
            Settings::default().translation_limits.output_kib,
            Ok(scratch.0.clone()),
        )
        .unwrap()
        .identity(Output::Range)
    };
    let service = plamo("http://127.0.0.1:11434/v1", "local");
    assert_eq!(service, plamo("http://127.0.0.1:11434/v1/", "local"));
    for other in [
        plamo("http://127.0.0.1:11435/v1", "local"),
        plamo("http://127.0.0.1:11434/v1", "other"),
    ] {
        assert_ne!(service, other);
    }
    let key = |identity: &Identity| Key::new(identity, "ja", "auto", Sent::Passage("Retry."));
    assert_ne!(
        key(&service),
        key(&compatible("http://127.0.0.1:11434/v1", "local", None))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn memory_keeps_the_most_recently_used_within_its_limits() {
    let store = store(
        json!({"translation_limits": {"output_kib": 1, "cache": {"entries": 2, "capacity_kib": 1}}}),
        None,
    );
    let (a, b, c) = (
        passage(&store, "a"),
        passage(&store, "b"),
        passage(&store, "c"),
    );
    store.put(a, translation("A"));
    store.put(b, translation("B"));
    assert_eq!(store.get(a).await, Some(translation("A")));
    store.put(c, translation("C"));
    // `b` was used least recently.
    assert_eq!(store.get(b).await, None);
    assert_eq!(store.get(a).await, Some(translation("A")));
    assert_eq!(store.get(c).await, Some(translation("C")));
    // One translation of the output limit fills the capacity.
    store.put(b, translation(&"B".repeat(1024)));
    assert_eq!(store.get(a).await, None);
    assert_eq!(store.get(c).await, None);
    assert!(store.get(b).await.is_some());
    // A longer one is not kept at all.
    store.put(a, translation(&"A".repeat(1025)));
    assert_eq!(store.get(a).await, None);
    assert!(store.get(b).await.is_some());
    // Nor content of another kind than the key's.
    store.put(a, Content::SameLanguage);
    assert_eq!(store.get(a).await, None);
    store.remove(&[b]);
    assert!(store.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn another_server_reads_what_one_wrote_to_disk() {
    let scratch = Scratch::new();
    let on = json!({"disk_cache": {"enabled": true}});
    let first = store(on.clone(), Some(&scratch.translations()));
    let entry = first.key(
        Output::Range,
        "ja",
        Sent::Term {
            term: "retain",
            line: "Retain the value.",
        },
    );
    let same = first.key(Output::Range, "ja", Sent::SameLanguage("こんにちは"));
    let part = first.key(Output::Document, "ja", Sent::Part("## Part"));
    let contents = [
        (passage(&first, "Retry."), translation("再試行する。")),
        (part, translation("## 部分")),
        (same, Content::SameLanguage),
        (
            entry,
            Content::Entry {
                text: "**retain** 保持する".into(),
                line: "retain — 保持する".into(),
            },
        ),
    ];
    for (key, content) in &contents {
        first.put(*key, content.clone());
    }
    drop(first);
    let second = store(on, Some(&scratch.translations()));
    assert!(second.is_empty());
    for (key, content) in &contents {
        assert_eq!(second.get(*key).await.as_ref(), Some(content));
    }
    // What it read stays in memory.
    assert!(!second.is_empty());
    let file = |key: &Key| fs::read_to_string(scratch.translations().join(key.hex())).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&file(&same)).unwrap(),
        json!({"v": 1, "kind": "same-language"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&file(&entry)).unwrap(),
        json!({"v": 1, "kind": "term", "text": "**retain** 保持する", "line": "retain — 保持する"})
    );
    // With `disk_cache` off, nothing is read or written.
    let off = store(json!({}), Some(&scratch.translations()));
    assert_eq!(off.get(same).await, None);
    let other = passage(&off, "Other.");
    off.put(other, translation("その他。"));
    assert!(!scratch.translations().join(other.hex()).exists());
}

#[cfg(unix)]
#[test]
fn files_and_their_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: PathBuf| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let new = Scratch::new();
    // A directory that was there before is made private too.
    let existing = Scratch::new();
    fs::create_dir(existing.translations()).unwrap();
    fs::set_permissions(existing.translations(), fs::Permissions::from_mode(0o755)).unwrap();
    for scratch in [new, existing] {
        let store = store(
            json!({"disk_cache": {"enabled": true}}),
            Some(&scratch.translations()),
        );
        let key = passage(&store, "Retry.");
        store.put(key, translation("再試行する。"));
        // Waits for the write.
        drop(store);
        assert_eq!(mode(scratch.translations()), 0o700);
        assert_eq!(mode(scratch.translations().join(key.hex())), 0o600);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unreadable_files_are_not_found_and_removed() {
    let scratch = Scratch::new();
    let directory = scratch.translations();
    let store = store(
        json!({"disk_cache": {"enabled": true}, "translation_limits": {"output_kib": 1}}),
        Some(&directory),
    );
    fs::create_dir_all(&directory).unwrap();
    let key = passage(&store, "Retry.");
    for written in [
        "not JSON".to_owned(),
        json!({"v": 2, "kind": "passage", "text": "再試行する。"}).to_string(),
        json!({"v": 1, "kind": "part", "text": "再試行する。"}).to_string(),
        json!({"v": 1, "kind": "same-language"}).to_string(),
        json!({"v": 1, "kind": "passage"}).to_string(),
        json!({"v": 1, "kind": "passage", "text": "再試行する。", "line": "再試行"}).to_string(),
        json!({"v": 1, "kind": "passage", "text": "訳".repeat(342)}).to_string(),
    ] {
        let path = directory.join(key.hex());
        fs::write(&path, &written).unwrap();
        assert_eq!(store.get(key).await, None, "{written}");
        assert!(!path.exists(), "{written}");
    }
    // A text of the output limit is read.
    fs::write(
        directory.join(key.hex()),
        json!({"v": 1, "kind": "passage", "text": "訳".repeat(341)}).to_string(),
    )
    .unwrap();
    assert!(store.get(key).await.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn translations_are_kept_in_memory_when_the_directory_cannot_be_made() {
    let scratch = Scratch::new();
    // A file stands where the directory would be.
    let blocked = scratch.0.join("file");
    fs::write(&blocked, "").unwrap();
    let store = store(
        json!({"disk_cache": {"enabled": true}}),
        Some(&blocked.join("translations")),
    );
    let key = passage(&store, "Retry.");
    store.put(key, translation("再試行する。"));
    assert_eq!(store.get(key).await, Some(translation("再試行する。")));
    store.remove(&[key]);
    store.clear();
}

/// Writes a file named `name` into `directory`, last used `age` ago.
fn aged(directory: &Path, name: &str, bytes: usize, age: Duration) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, "x".repeat(bytes)).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
    path
}

#[test]
fn cleaning_removes_old_files_then_the_least_recently_used_beyond_the_size() {
    let scratch = Scratch::new();
    let directory = scratch.translations();
    fs::create_dir_all(&directory).unwrap();
    let day = Duration::from_secs(24 * 60 * 60);
    let hash = |digit: char| digit.to_string().repeat(64);
    let expired = aged(&directory, &hash('1'), 10, day * 31);
    let oldest = aged(&directory, &hash('2'), 40, day * 3);
    let older = aged(&directory, &hash('3'), 40, day * 2);
    let recent = aged(&directory, &hash('4'), 40, day);
    let temporary = format!(".{}-{}.tmp", hash('5'), Uuid::new_v4());
    let abandoned = aged(&directory, &temporary, 10, Duration::from_secs(2 * 60 * 60));
    let temporary = format!(".{}-{}.tmp", hash('6'), Uuid::new_v4());
    let writing = aged(&directory, &temporary, 10, Duration::from_secs(60));
    // Names that the server does not write stay, however old.
    let others = [
        aged(&directory, &hash('A'), 10, day * 400),
        aged(&directory, &"7".repeat(63), 10, day * 400),
        aged(&directory, "notes.txt", 10, day * 400),
        aged(&directory, &format!(".{}-x.tmp", hash('8')), 10, day * 400),
    ];
    Disk::new(directory).clean(day * 30, 80, SystemTime::now());
    for (path, exists) in [
        (&expired, false),
        (&oldest, false),
        (&older, true),
        (&recent, true),
        (&abandoned, false),
        (&writing, true),
    ] {
        assert_eq!(path.exists(), exists, "{}", path.display());
    }
    assert!(others.iter().all(|path| path.exists()));
}

/// Sets when the file at `path` was last used to `age` ago.
fn age(path: &Path, age: Duration) -> SystemTime {
    let modified = SystemTime::now() - age;
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    modified
}

#[tokio::test(flavor = "current_thread")]
async fn a_used_translation_is_marked_as_used_on_disk() {
    let scratch = Scratch::new();
    let directory = scratch.translations();
    let on = json!({"disk_cache": {"enabled": true}});
    let writer = store(on.clone(), Some(&directory));
    let key = passage(&writer, "Retry.");
    writer.put(key, translation("再試行する。"));
    drop(writer);
    let path = directory.join(key.hex());
    let hour = Duration::from_secs(60 * 60);
    // Read from disk.
    let modified = age(&path, hour * 2);
    let reader = store(on, Some(&directory));
    assert!(reader.get(key).await.is_some());
    assert!(fs::metadata(&path).unwrap().modified().unwrap() > modified);
    // Found in memory within an hour of the last mark: not marked again.
    let modified = age(&path, hour * 2);
    assert!(reader.get(key).await.is_some());
    // A read goes after every operation sent before it.
    assert_eq!(reader.get(passage(&reader, "Other.")).await, None);
    let last_used = || fs::metadata(&path).unwrap().modified().unwrap();
    assert!(last_used() < SystemTime::now() - hour);
    // Found in memory an hour after it.
    let earlier = Instant::now().checked_sub(hour).unwrap();
    reader.memory().entries.get_mut(&key).unwrap().touched = earlier;
    assert!(reader.get(key).await.is_some());
    drop(reader);
    assert!(last_used() > modified);
}

#[tokio::test(flavor = "current_thread")]
async fn a_read_that_a_change_overtook_is_not_kept_in_memory() {
    let scratch = Scratch::new();
    let directory = scratch.translations();
    let on = json!({"disk_cache": {"enabled": true}});
    let writer = store(on.clone(), Some(&directory));
    let key = passage(&writer, "Retry.");
    writer.put(key, translation("古い訳"));
    drop(writer);
    let store = store(on, Some(&directory));
    let reading = tokio::spawn({
        let store = store.clone();
        async move { store.get(key).await }
    });
    // The read is sent before the write.
    tokio::task::yield_now().await;
    store.put(key, translation("新しい訳"));
    assert_eq!(reading.await.unwrap(), Some(translation("古い訳")));
    assert_eq!(store.get(key).await, Some(translation("新しい訳")));
    drop(store);
    let reader = store_on(&directory);
    assert_eq!(reader.get(key).await, Some(translation("新しい訳")));
}

fn store_on(directory: &Path) -> Store {
    store(json!({"disk_cache": {"enabled": true}}), Some(directory))
}

#[tokio::test(flavor = "current_thread")]
async fn only_clearing_touches_the_disk_with_disk_cache_off() {
    let scratch = Scratch::new();
    let directory = scratch.translations();
    let writer = store_on(&directory);
    let key = passage(&writer, "Retry.");
    writer.put(key, translation("再試行する。"));
    drop(writer);
    let path = directory.join(key.hex());
    let off = store(json!({}), Some(&directory));
    off.put(key, translation("別の訳"));
    off.remove(&[key]);
    drop(off);
    assert!(path.exists());
    assert_eq!(
        store_on(&directory).get(key).await,
        Some(translation("再試行する。"))
    );
    // Files of other names stay, and with them the directory.
    fs::write(directory.join("notes.txt"), "").unwrap();
    let temporary = directory.join(format!(".{}-{}.tmp", key.hex(), Uuid::new_v4()));
    fs::write(&temporary, "").unwrap();
    let off = store(json!({}), Some(&directory));
    off.clear();
    drop(off);
    assert!(!path.exists());
    assert!(!temporary.exists());
    assert!(directory.join("notes.txt").exists());
    // Without them, the directory goes too.
    fs::remove_file(directory.join("notes.txt")).unwrap();
    let writer = store_on(&directory);
    writer.put(key, translation("再試行する。"));
    assert_eq!(writer.get(key).await, Some(translation("再試行する。")));
    writer.clear();
    assert_eq!(writer.get(key).await, None);
    assert!(!directory.exists());
    // A later write makes the directory again.
    writer.put(key, translation("再試行する。"));
    drop(writer);
    assert!(path.exists());
}
