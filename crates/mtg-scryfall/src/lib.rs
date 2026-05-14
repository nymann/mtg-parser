//! Scryfall API adapter: fetches cards by set with a polite, on-disk
//! cache. Outermost layer of the parser; the core (`mtg-grammar`,
//! `mtg-semantic`) does not depend on it.
//!
//! See `ARCHITECTURE.md` for the caching contract and freshness rule.

mod cache;
mod card;
mod client;
mod date;

pub use cache::{CachedSet, CACHE_SCHEMA_VERSION, REFRESH_WINDOW_DAYS};
pub use card::{Card, Layout};
pub use client::ScryfallClient;
pub use date::today_iso;
