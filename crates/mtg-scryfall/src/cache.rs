use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::date::days_between_iso;

pub const CACHE_SCHEMA_VERSION: u32 = 1;
pub const REFRESH_WINDOW_DAYS: i64 = 30;

/// Cached payload for a single set. One file per set keyed by lowercased
/// set code. See ARCHITECTURE.md for the freshness rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSet {
    /// Schema version. Bumping invalidates older payloads automatically.
    #[serde(rename = "_v")]
    pub schema_version: u32,
    pub set_code: String,
    pub released_at: Option<String>,
    pub fetched_at: String,
    pub cards: Vec<Card>,
}

impl CachedSet {
    /// A set's cache is fresh once its `released_at` is more than
    /// [`REFRESH_WINDOW_DAYS`] in the past — MTG sets are frozen after
    /// release. Sets in spoiler season (future or recent release) are
    /// always considered stale so we re-fetch on every run.
    pub fn is_fresh(&self, today_iso: &str) -> bool {
        if self.schema_version != CACHE_SCHEMA_VERSION {
            return false;
        }
        let Some(released) = self.released_at.as_deref() else {
            return false;
        };
        match days_between_iso(released, today_iso) {
            Some(days) => days >= REFRESH_WINDOW_DAYS,
            None => false,
        }
    }
}
