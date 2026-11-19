//! The keys that stored translations are found by: what was sent, and the
//! settings that change what comes back.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The version of what a key stands for. Raise it when a change to what is
/// sent, such as the instructions in `providers/prompt.rs` or
/// `providers/dictionary.rs` or the segments of `providers/segments.rs`, changes
/// the translations that come back, so that translations stored before the
/// change are no longer found.
pub(crate) const FORMAT_VERSION: u32 = 1;

/// What a provider sends for one output that changes what comes back, other
/// than the text and the languages: its name in the settings, then named
/// settings, such as the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    fields: Vec<(&'static str, String)>,
}

impl Identity {
    /// The identity of the provider that the settings name `provider`.
    pub fn new(provider: &str) -> Self {
        Self {
            fields: vec![("provider", provider.to_owned())],
        }
    }

    /// Adds a setting, written as its JSON value.
    pub fn with(mut self, name: &'static str, value: impl Serialize) -> Self {
        let value = match serde_json::to_value(value) {
            Ok(serde_json::Value::String(text)) => text,
            Ok(value) => value.to_string(),
            Err(_) => String::new(),
        };
        self.fields.push((name, value));
        self
    }

    /// Adds a setting if it is present.
    pub fn with_some(self, name: &'static str, value: Option<impl Serialize>) -> Self {
        match value {
            Some(value) => self.with(name, value),
            None => self,
        }
    }
}

/// What a stored translation is.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The translation of a whole text.
    Passage,
    /// The translation of one part of a text sent in parts, whose
    /// instructions differ.
    Part,
    /// That an automatic request found the text already in the target
    /// language.
    SameLanguage,
    /// A dictionary entry.
    Term,
}

impl Kind {
    /// The name of the kind in a stored file.
    pub fn name(self) -> &'static str {
        match self {
            Self::Passage => "passage",
            Self::Part => "part",
            Self::SameLanguage => "same-language",
            Self::Term => "term",
        }
    }
}

/// What was sent, by the kind of result it is stored as.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Sent<'a> {
    Passage(&'a str),
    Part(&'a str),
    SameLanguage(&'a str),
    Term { term: &'a str, line: &'a str },
}

impl Sent<'_> {
    fn kind(self) -> Kind {
        match self {
            Self::Passage(_) => Kind::Passage,
            Self::Part(_) => Kind::Part,
            Self::SameLanguage(_) => Kind::SameLanguage,
            Self::Term { .. } => Kind::Term,
        }
    }
}

/// The key of a stored translation: a SHA-256 of what it was made from.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct Key {
    kind: Kind,
    hash: [u8; 32],
}

impl Key {
    /// The key of what `identity` gives for `sent`, with the languages as the
    /// settings write them.
    pub fn new(
        identity: &Identity,
        target_language: &str,
        source_language: &str,
        sent: Sent<'_>,
    ) -> Self {
        let kind = sent.kind();
        let mut digest = Sha256::new();
        // Each field is written with the lengths of its name and value, so
        // that no two sets of fields write the same bytes.
        let mut field = |name: &str, value: &str| {
            for part in [name, value] {
                digest.update((part.len() as u64).to_le_bytes());
                digest.update(part.as_bytes());
            }
        };
        field("format", &FORMAT_VERSION.to_string());
        for (name, value) in &identity.fields {
            field(name, value);
        }
        field("target", target_language);
        field("source", source_language);
        field("kind", kind.name());
        match sent {
            Sent::Passage(text) | Sent::Part(text) | Sent::SameLanguage(text) => {
                field("text", text);
            }
            Sent::Term { term, line } => {
                field("term", term);
                field("line", line);
            }
        }
        Self {
            kind,
            hash: digest.finalize().into(),
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The hash in 64 lowercase hexadecimal digits.
    pub fn hex(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(64);
        for byte in self.hash {
            hex.push(DIGITS[usize::from(byte >> 4)] as char);
            hex.push(DIGITS[usize::from(byte & 0xf)] as char);
        }
        hex
    }
}
