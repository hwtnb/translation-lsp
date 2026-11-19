use serde::Deserialize;

use super::error::{ConfigError, bounded_number};

/// `document_split`: whether a translation opened in a tab is sent in parts
/// at once, and how.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DocumentSplit {
    /// Without it, the other fields are checked but do nothing.
    pub enabled: bool,
    /// The requests that one translation in a tab sends at once.
    pub parallel: usize,
    /// The size a part aims at, in KiB.
    pub chunk_kib: usize,
}

impl Default for DocumentSplit {
    fn default() -> Self {
        Self {
            enabled: false,
            parallel: 4,
            chunk_kib: 8,
        }
    }
}

impl DocumentSplit {
    pub(super) fn validate(&self) -> Result<(), ConfigError> {
        bounded_number("document_split.parallel", self.parallel as u64, 1, 8)?;
        bounded_number(
            "document_split.chunk_kib",
            self.chunk_kib as u64,
            1,
            16 * 1024,
        )
    }

    /// The size a part aims at, in bytes.
    pub fn chunk_bytes(&self) -> usize {
        self.chunk_kib * 1024
    }
}
