use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::cache::{CachedSet, CACHE_SCHEMA_VERSION};
use crate::card::{Card, Layout};
use crate::date::today_iso;

const SCRYFALL_BASE: &str = "https://api.scryfall.com";
const USER_AGENT: &str = concat!("mtg-parser/", env!("CARGO_PKG_VERSION"));
const REQUEST_DELAY: Duration = Duration::from_millis(150);
const MAX_RETRIES: u32 = 5;

/// Polite, caching client for the Scryfall public API.
///
/// Caches per-set JSON under `~/.cache/scryfall/<code>.json` (or the
/// platform-equivalent via [`dirs::cache_dir`]). One cached payload is
/// considered fresh once the set's `released_at` is more than 30 days
/// in the past — see [`crate::cache::CachedSet::is_fresh`].
pub struct ScryfallClient {
    cache_dir: PathBuf,
    http: Client,
    last_request: Mutex<Instant>,
}

impl ScryfallClient {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("no platform cache directory available"))?
            .join("scryfall");
        Self::with_cache_dir(cache_dir)
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("create cache dir {}", cache_dir.display()))?;
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            cache_dir,
            http,
            last_request: Mutex::new(Instant::now() - REQUEST_DELAY),
        })
    }

    /// Cards for a set. Uses the cache if fresh; otherwise re-fetches.
    pub fn cards_in_set(&self, code: &str) -> Result<Vec<Card>> {
        self.load_set(code, false)
    }

    /// Force a re-fetch, ignoring cache freshness.
    pub fn refresh_set(&self, code: &str) -> Result<Vec<Card>> {
        self.load_set(code, true)
    }

    /// Paper core/expansion sets, ordered by release date then set code.
    pub fn paper_expansion_sets(&self) -> Result<Vec<SetSummary>> {
        let page: SetsPage = self
            .get_json(&format!("{SCRYFALL_BASE}/sets"))
            .context("fetch /sets")?;
        let mut sets: Vec<SetSummary> = page
            .data
            .into_iter()
            .filter(|s| {
                !s.digital
                    && s.released_at.is_some()
                    && matches!(s.set_type.as_str(), "core" | "expansion")
            })
            .map(|s| SetSummary {
                code: s.code.to_lowercase(),
                name: s.name,
                released_at: s.released_at.expect("filtered above"),
            })
            .collect();
        sets.sort_by(|a, b| {
            a.released_at
                .cmp(&b.released_at)
                .then_with(|| a.code.cmp(&b.code))
        });
        Ok(sets)
    }

    fn load_set(&self, code: &str, force_refresh: bool) -> Result<Vec<Card>> {
        let path = self.cache_path(code);
        if !force_refresh {
            if let Some(fresh) = read_fresh_cache(&path) {
                return Ok(fresh.cards);
            }
        }
        match self.fetch_set(code) {
            Ok(payload) => {
                let json =
                    serde_json::to_string_pretty(&payload).context("serialize cached set")?;
                std::fs::write(&path, json)
                    .with_context(|| format!("write cache {}", path.display()))?;
                Ok(payload.cards)
            }
            Err(e) => {
                if let Some(stale) = read_any_cache(&path) {
                    eprintln!(
                        "warning: refresh for {code} failed ({e:#}); using stale cache from {}",
                        stale.fetched_at,
                    );
                    return Ok(stale.cards);
                }
                Err(e)
            }
        }
    }

    fn cache_path(&self, code: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", code.to_lowercase()))
    }

    fn fetch_set(&self, code: &str) -> Result<CachedSet> {
        let code_lc = code.to_lowercase();
        let meta: SetMeta = self
            .get_json(&format!("{SCRYFALL_BASE}/sets/{code_lc}"))
            .with_context(|| format!("fetch /sets/{code_lc}"))?;
        let initial = reqwest::Url::parse_with_params(
            &format!("{SCRYFALL_BASE}/cards/search"),
            &[
                ("q", format!("set:{code_lc} -is:rebalanced").as_str()),
                ("unique", "cards"),
                ("order", "name"),
            ],
        )
        .context("build search URL")?;

        let mut cards = Vec::new();
        let mut next = Some(initial.to_string());
        while let Some(url) = next {
            let page: SearchPage = self.get_json(&url).context("fetch /cards/search page")?;
            for raw in page.data {
                cards.push(raw_to_card(raw, &code_lc));
            }
            next = if page.has_more { page.next_page } else { None };
        }

        Ok(CachedSet {
            schema_version: CACHE_SCHEMA_VERSION,
            set_code: code_lc,
            released_at: meta.released_at,
            fetched_at: today_iso(),
            cards,
        })
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        for attempt in 0..MAX_RETRIES {
            self.throttle();
            let resp = self
                .http
                .get(url)
                .send()
                .with_context(|| format!("GET {url}"))?;
            let status = resp.status();
            if status.is_success() {
                return resp
                    .json()
                    .with_context(|| format!("decode JSON from {url}"));
            }
            if status.as_u16() == 429 && attempt + 1 < MAX_RETRIES {
                let wait = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_secs(1u64 << attempt));
                std::thread::sleep(wait);
                continue;
            }
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("HTTP {status} for {url}: {body}"));
        }
        Err(anyhow!("retries exhausted for {url}"))
    }

    fn throttle(&self) {
        let mut last = self.last_request.lock().expect("mutex not poisoned");
        let since = last.elapsed();
        if since < REQUEST_DELAY {
            std::thread::sleep(REQUEST_DELAY - since);
        }
        *last = Instant::now();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSummary {
    pub code: String,
    pub name: String,
    pub released_at: String,
}

fn read_fresh_cache(path: &std::path::Path) -> Option<CachedSet> {
    let cached = read_any_cache(path)?;
    if cached.is_fresh(&today_iso()) {
        Some(cached)
    } else {
        None
    }
}

fn read_any_cache(path: &std::path::Path) -> Option<CachedSet> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[derive(Deserialize)]
struct SetMeta {
    released_at: Option<String>,
}

#[derive(Deserialize)]
struct SetsPage {
    data: Vec<RawSet>,
}

#[derive(Deserialize)]
struct RawSet {
    code: String,
    name: String,
    released_at: Option<String>,
    set_type: String,
    #[serde(default)]
    digital: bool,
}

#[derive(Deserialize)]
struct SearchPage {
    data: Vec<RawCard>,
    #[serde(default)]
    has_more: bool,
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct RawCard {
    name: String,
    collector_number: String,
    #[serde(default)]
    oracle_text: String,
    #[serde(default)]
    mana_cost: String,
    layout: Layout,
    set: Option<String>,
}

fn raw_to_card(raw: RawCard, fallback_set: &str) -> Card {
    Card {
        name: raw.name,
        set_code: raw.set.unwrap_or_else(|| fallback_set.to_string()),
        collector_number: raw.collector_number,
        oracle_text: raw.oracle_text,
        mana_cost: raw.mana_cost,
        layout: raw.layout,
    }
}
