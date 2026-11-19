use std::time::Duration;

use serde::Deserialize;

use super::error::{ConfigError, bounded_number};

/// `disk_cache`: whether completed translations are also kept on disk, to be
/// used after a restart, and for how long.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DiskCache {
    /// Without it, `days` is checked but does nothing.
    pub enabled: bool,
    /// The days a translation stays on disk without being used.
    pub days: u64,
}

impl Default for DiskCache {
    fn default() -> Self {
        Self {
            enabled: false,
            days: 30,
        }
    }
}

impl DiskCache {
    pub(super) fn validate(&self) -> Result<(), ConfigError> {
        bounded_number("disk_cache.days", self.days, 1, 365)
    }

    /// How long a translation stays on disk without being used.
    pub fn unused_for(&self) -> Duration {
        Duration::from_secs(self.days * 24 * 60 * 60)
    }
}
