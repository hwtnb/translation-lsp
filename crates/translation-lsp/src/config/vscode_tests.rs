//! The settings and languages of the VS Code extension, which keeps them in
//! `vscode-extension/package.json` and `vscode-extension/src/providers.json`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{Settings, features::Feature, hover::HoverFormat};
use crate::language::Language;

const MANIFEST: &str = include_str!("../../../../vscode-extension/package.json");
const PROVIDERS: &str = include_str!("../../../../vscode-extension/src/providers.json");
const ZED_MANIFEST: &str = include_str!("../../../../zed-extension/extension.toml");

/// The VS Code language IDs outside `extension.toml` that `Language::of_document`
/// maps to a registered language.
const REMAPPED_LANGUAGES: [&str; 10] = [
    "javascriptreact",
    "kotlinscript",
    "proto3",
    "terraform-stack",
    "terraform-deploy",
    "terraform-test",
    "terraform-mock",
    "terraform-search",
    "terraform-policy",
    "terraform-policytest",
];
/// The section of the VS Code settings of the extension.
const SECTION: &str = "translationLsp.";
/// The VS Code setting that names the server executable, which the server
/// never sees.
const SERVER_PATH: &str = "translationLsp.server.path";

/// The settings inside `provider` that some provider takes, each with a value
/// that every provider that takes it accepts.
const PROVIDER_FIELDS: [(&str, &str); 9] = [
    ("model", r#""synthetic-model""#),
    ("effort", r#""low""#),
    ("thinking", "true"),
    ("speed", r#""fast""#),
    ("base_url", r#""http://127.0.0.1:9/v1""#),
    ("document.model", r#""synthetic-model""#),
    ("document.effort", r#""low""#),
    ("document.thinking", "true"),
    ("document.speed", r#""fast""#),
];

/// Every setting of the server with its default, or `None` for one without a
/// single default: `features.languages` and those of the provider.
fn server_settings() -> Vec<(String, Option<Value>)> {
    let defaults = Settings::default();
    let kib = |limit: super::limits::SizeLimit| json!(limit.bytes() / 1024);
    let limits = defaults.translation_limits;
    let mut settings: Vec<(String, Option<Value>)> = [
        ("target_language", json!(defaults.target_language)),
        ("source_language", json!(defaults.source_language)),
        ("hover_timeout_ms", json!(defaults.hover_timeout_ms)),
        ("selection_timeout_ms", json!(defaults.selection_timeout_ms)),
        ("document_timeout_ms", json!(defaults.document_timeout_ms)),
        (
            "selection_debounce_ms",
            json!(defaults.selection_debounce_ms),
        ),
        (
            "comment_parse_timeout_ms",
            json!(defaults.comment_parse_timeout_ms),
        ),
        ("hover.header", json!(defaults.hover.header)),
        (
            "hover.format",
            json!(match defaults.hover.format {
                HoverFormat::Markdown => "markdown",
                HoverFormat::Text => "text",
            }),
        ),
        (
            "translation_limits.input_kib.hover",
            kib(limits.input_kib.hover),
        ),
        (
            "translation_limits.input_kib.document",
            kib(limits.input_kib.document),
        ),
        ("translation_limits.output_kib", kib(limits.output_kib)),
        (
            "translation_limits.work.admitted",
            json!(limits.work.admitted),
        ),
        (
            "translation_limits.work.concurrent",
            json!(limits.work.concurrent),
        ),
        (
            "translation_limits.work.documents",
            json!(limits.work.documents),
        ),
        (
            "translation_limits.work.automatic",
            json!(limits.work.automatic),
        ),
        (
            "translation_limits.work.hover_waiters",
            json!(limits.work.hover_waiters),
        ),
        (
            "translation_limits.cache.entries",
            json!(limits.cache.entries),
        ),
        (
            "translation_limits.cache.capacity_kib",
            kib(limits.cache.capacity_kib),
        ),
        (
            "document_split.enabled",
            json!(defaults.document_split.enabled),
        ),
        (
            "document_split.parallel",
            json!(defaults.document_split.parallel),
        ),
        (
            "document_split.chunk_kib",
            json!(defaults.document_split.chunk_kib),
        ),
        ("disk_cache.enabled", json!(defaults.disk_cache.enabled)),
        ("disk_cache.days", json!(defaults.disk_cache.days)),
    ]
    .into_iter()
    .map(|(name, default)| (name.to_owned(), Some(default)))
    .collect();
    for (feature, name) in [
        (Feature::CommentHover, "comment_hover"),
        (Feature::AutomaticSelection, "automatic_selection"),
        (Feature::SelectionAction, "selection_action"),
        (Feature::DocumentAction, "document_action"),
        (Feature::CommentLens, "comment_lens"),
    ] {
        let default = json!(defaults.features.enables(feature, None));
        settings.push((format!("features.{name}"), Some(default)));
    }
    settings.push(("features.languages".into(), None));
    settings.push(("provider.name".into(), None));
    for (field, _) in PROVIDER_FIELDS {
        settings.push((format!("provider.{field}"), None));
    }
    settings
}

/// The VS Code settings of the extension, by the server setting each stands
/// for: its name without the section, with each capital letter written as `_`
/// and the letter in lowercase.
fn vscode_settings() -> BTreeMap<String, Value> {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    manifest["contributes"]["configuration"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["properties"].as_object().unwrap().clone())
        .filter(|(name, _)| name != SERVER_PATH)
        .map(|(name, schema)| {
            let name = name.strip_prefix(SECTION).unwrap();
            let mut server = String::new();
            for character in name.chars() {
                if character.is_ascii_uppercase() {
                    server.push('_');
                }
                server.push(character.to_ascii_lowercase());
            }
            (server, schema)
        })
        .collect()
}

fn providers() -> BTreeMap<String, Vec<String>> {
    serde_json::from_str(PROVIDERS).unwrap()
}

#[test]
fn every_server_setting_has_one_vs_code_setting_with_its_default() {
    let vscode = vscode_settings();
    let server = server_settings();
    assert_eq!(
        vscode.keys().cloned().collect::<BTreeSet<_>>(),
        server.iter().map(|(name, _)| name.clone()).collect(),
    );
    for (name, default) in server {
        let schema = &vscode[&name];
        match default {
            Some(default) => assert_eq!(schema["default"], default, "{name}"),
            // Nothing written leaves the choice to the server.
            None => assert!(
                ["", "auto"].contains(&schema["default"].as_str().unwrap_or("")),
                "{name}: {schema}"
            ),
        }
    }
}

/// Every VS Code setting but those of the provider, written with its default
/// under the server's name, is accepted: the names are the server's.
#[test]
fn the_defaults_of_the_vs_code_settings_are_accepted_under_their_names() {
    let mut options = Map::new();
    for (name, schema) in vscode_settings() {
        if name.starts_with("provider.") {
            continue;
        }
        let mut parts: Vec<&str> = name.split('.').collect();
        let last = parts.pop().unwrap();
        let mut inner = &mut options;
        for part in parts {
            inner = inner
                .entry(part)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .unwrap();
        }
        inner.insert(last.into(), schema["default"].clone());
    }
    let options = Value::Object(options);
    if let Err(error) = Settings::from_options(Some(options.clone())) {
        panic!("{options}: {error}");
    }
}

/// Settings that the extension builds as its table says are accepted, and a
/// setting that the table leaves out of a provider is not: the extension
/// would otherwise keep back a setting that the provider takes.
#[test]
fn each_provider_takes_the_settings_of_its_row_and_no_other() {
    let providers = providers();
    let vscode = vscode_settings();
    let mut names: Vec<_> = vscode["provider.name"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names.remove(0), "");
    assert_eq!(
        names.into_iter().collect::<BTreeSet<_>>(),
        providers.keys().cloned().collect()
    );
    let options = |name: &str, fields: &[&str]| {
        let mut provider = Map::new();
        provider.insert("name".into(), json!(name));
        for field in fields {
            let (_, value) = PROVIDER_FIELDS.iter().find(|(f, _)| f == field).unwrap();
            let value: Value = serde_json::from_str(value).unwrap();
            match field.strip_prefix("document.") {
                Some(inner) => {
                    provider
                        .entry("document")
                        .or_insert_with(|| json!({}))
                        .as_object_mut()
                        .unwrap()
                        .insert(inner.into(), value);
                }
                None => {
                    provider.insert((*field).into(), value);
                }
            }
        }
        json!({ "provider": provider })
    };
    for (name, fields) in &providers {
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        for field in &fields {
            assert!(
                PROVIDER_FIELDS.iter().any(|(f, _)| f == field),
                "{name}: {field}"
            );
        }
        let accepted = options(name, &fields);
        if let Err(error) = Settings::from_options(Some(accepted.clone())) {
            panic!("{accepted}: {error}");
        }
        for (other, _) in PROVIDER_FIELDS {
            if fields.contains(&other) {
                continue;
            }
            let mut with = fields.clone();
            with.push(other);
            if other.starts_with("document.") && !with.contains(&"document.model") {
                with.push("document.model");
            }
            let rejected = options(name, &with);
            assert!(
                Settings::from_options(Some(rejected.clone())).is_err(),
                "{rejected}"
            );
        }
    }
}

#[test]
fn the_extension_starts_the_server_only_for_languages_it_reads() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let languages: Vec<_> = manifest["activationEvents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event.as_str().unwrap().strip_prefix("onLanguage:"))
        .collect();
    let unique: BTreeSet<_> = languages.iter().copied().collect();
    assert_eq!(unique.len(), languages.len());

    let zed_languages: BTreeSet<_> = ZED_MANIFEST
        .lines()
        .skip_while(|line| *line != "[language_servers.translation-lsp.language_ids]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once(" = "))
        .map(|(_, language_id)| language_id.trim_matches('"'))
        .collect();
    for language in &zed_languages {
        assert!(
            unique.contains(language),
            "missing from VS Code activationEvents: {language}"
        );
    }

    let extra: BTreeSet<_> = unique.difference(&zed_languages).copied().collect();
    let expected_remapped: BTreeSet<_> = REMAPPED_LANGUAGES.into_iter().collect();
    assert_eq!(extra, expected_remapped);

    for language in languages {
        assert!(Language::of_document(language).is_some(), "{language}");
    }
    for language in REMAPPED_LANGUAGES {
        assert!(Language::from_id(language).is_none(), "{language}");
        assert!(Language::of_document(language).is_some(), "{language}");
    }
}
