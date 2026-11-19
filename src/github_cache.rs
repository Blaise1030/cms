//! A small cache for GitHub REST GETs made with a token.
//!
//! - A response younger than the TTL is reused without a request.
//! - An older one is revalidated with `If-None-Match` / `If-Modified-Since`; GitHub answers
//!   `304 Not Modified` without counting it against the rate limit, and the stored body is
//!   reused.
//! - Concurrent identical GETs share one request: the second waits for the first, then
//!   finds its fresh entry.
//! - Writes drop every entry for the repository they touched.
//!
//! Entries are keyed by the caller's identity (a hash of the token) and the full URL, so two
//! accounts never see each other's responses. Only 2xx bodies are stored, and at most
//! [`CAPACITY`] of them (least recently used evicted first).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;

/// How long a response is reused without asking GitHub at all.
pub const TTL: Duration = Duration::from_secs(15);
/// Most responses kept at once.
pub const CAPACITY: usize = 500;

struct Entry {
    url: String,
    body: Bytes,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched: Instant,
    used: u64,
}

#[derive(Default)]
struct State {
    entries: HashMap<String, Entry>,
    clock: u64,
}

/// What the cache holds for a key.
pub enum Lookup {
    /// Young enough to use as is.
    Fresh(Bytes),
    /// Stored, but must be revalidated with these validators (either may be absent).
    Stale {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Miss,
}

pub struct GetCache {
    ttl: Mutex<Duration>,
    state: Mutex<State>,
    flights: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for GetCache {
    fn default() -> Self {
        Self::new(TTL)
    }
}

impl GetCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl: Mutex::new(ttl),
            state: Mutex::default(),
            flights: tokio::sync::Mutex::default(),
        }
    }
}

/// Held while one GET for a key is in flight; identical GETs wait on it.
pub struct Flight {
    key: String,
    lock: Arc<tokio::sync::Mutex<()>>,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl GetCache {
    /// The cache key for `url` fetched with `token`. The token itself is not kept.
    pub fn key(token: &str, url: &str) -> String {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        token.hash(&mut h);
        format!("{:016x} {url}", h.finish())
    }

    #[cfg(test)]
    pub fn set_ttl(&self, ttl: Duration) {
        *self.ttl.lock().unwrap() = ttl;
    }

    /// Waits until no other GET for `key` is in flight, then claims it.
    pub async fn flight(&self, key: &str) -> Flight {
        let lock = {
            let mut flights = self.flights.lock().await;
            flights
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let guard = lock.clone().lock_owned().await;
        Flight {
            key: key.to_string(),
            lock,
            _guard: guard,
        }
    }

    /// Releases a flight, forgetting its lock when nobody else waits on it.
    pub async fn land(&self, flight: Flight) {
        let Flight { key, lock, _guard } = flight;
        drop(_guard);
        let mut flights = self.flights.lock().await;
        // The map and `lock` are the only holders: nobody is waiting.
        if Arc::strong_count(&lock) <= 2 {
            flights.remove(&key);
        }
    }

    pub fn lookup(&self, key: &str) -> Lookup {
        let ttl = *self.ttl.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        state.clock += 1;
        let now = state.clock;
        let Some(entry) = state.entries.get_mut(key) else {
            return Lookup::Miss;
        };
        entry.used = now;
        if entry.fetched.elapsed() < ttl {
            Lookup::Fresh(entry.body.clone())
        } else {
            Lookup::Stale {
                etag: entry.etag.clone(),
                last_modified: entry.last_modified.clone(),
            }
        }
    }

    /// GitHub said the stored body is still current: reuse it and restart its TTL.
    pub fn revalidated(&self, key: &str) -> Option<Bytes> {
        let mut state = self.state.lock().unwrap();
        state.clock += 1;
        let now = state.clock;
        let entry = state.entries.get_mut(key)?;
        entry.fetched = Instant::now();
        entry.used = now;
        Some(entry.body.clone())
    }

    /// Stores a 2xx body for `url` under `key`.
    pub fn store(
        &self,
        key: &str,
        url: &str,
        body: Bytes,
        etag: Option<String>,
        last_modified: Option<String>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.clock += 1;
        let used = state.clock;
        state.entries.insert(
            key.to_string(),
            Entry {
                url: url.to_string(),
                body,
                etag,
                last_modified,
                fetched: Instant::now(),
                used,
            },
        );
        while state.entries.len() > CAPACITY {
            let Some(oldest) = state
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            state.entries.remove(&oldest);
        }
    }

    /// Drops every entry whose URL is `prefix` or lies under `prefix/`.
    pub fn invalidate(&self, prefix: &str) {
        let prefix = prefix.trim_end_matches('/');
        let under = format!("{prefix}/");
        let query = format!("{prefix}?");
        self.state.lock().unwrap().entries.retain(|_, e| {
            !(e.url == prefix || e.url.starts_with(&under) || e.url.starts_with(&query))
        });
    }

    pub fn clear(&self) {
        self.state.lock().unwrap().entries.clear();
    }

    #[cfg(test)]
    pub fn entries(&self) -> usize {
        self.state.lock().unwrap().entries.len()
    }
}

/// The repository part (`/repos/{owner}/{name}`) of an API path, if it names one.
pub fn repo_prefix(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/repos/")?;
    let mut parts = rest.splitn(3, '/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let name = parts.next().filter(|s| !s.is_empty())?;
    let name = name.split('?').next().unwrap_or(name);
    let len = "/repos/".len() + owner.len() + 1 + name.len();
    Some(&path[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> Bytes {
        Bytes::from(s.to_string())
    }

    #[test]
    fn fresh_then_stale_then_revalidated() {
        let c = GetCache::default();
        let k = GetCache::key("t", "u");
        assert!(matches!(c.lookup(&k), Lookup::Miss));
        c.store(&k, "u", body("a"), Some("\"e1\"".into()), None);
        assert!(matches!(c.lookup(&k), Lookup::Fresh(b) if b == "a"));
        c.set_ttl(Duration::ZERO);
        assert!(matches!(c.lookup(&k), Lookup::Stale { etag: Some(e), .. } if e == "\"e1\""));
        assert_eq!(c.revalidated(&k).unwrap(), "a");
    }

    #[test]
    fn keys_differ_by_token_and_hide_it() {
        let a = GetCache::key("tok-a", "https://api/x");
        let b = GetCache::key("tok-b", "https://api/x");
        assert_ne!(a, b);
        assert!(!a.contains("tok-a"));
    }

    #[test]
    fn evicts_the_least_recently_used_past_capacity() {
        let c = GetCache::default();
        for n in 0..CAPACITY {
            c.store(&format!("k{n}"), "u", body("x"), None, None);
        }
        // Touch k0 so k1 is now the oldest.
        assert!(matches!(c.lookup("k0"), Lookup::Fresh(_)));
        c.store("new", "u", body("x"), None, None);
        assert_eq!(c.entries(), CAPACITY);
        assert!(matches!(c.lookup("k0"), Lookup::Fresh(_)));
        assert!(matches!(c.lookup("k1"), Lookup::Miss));
    }

    #[test]
    fn invalidates_a_repository_and_nothing_else() {
        let c = GetCache::default();
        let api = "https://api.github.com";
        for (k, url) in [
            ("a", format!("{api}/repos/o/site/pulls?state=open&page=1")),
            ("b", format!("{api}/repos/o/site/issues/3/comments")),
            ("c", format!("{api}/repos/o/site")),
            ("d", format!("{api}/repos/o/site-two/pulls")),
            ("e", format!("{api}/user")),
        ] {
            c.store(k, &url, body("x"), None, None);
        }
        c.invalidate(&format!("{api}/repos/o/site"));
        assert!(matches!(c.lookup("a"), Lookup::Miss));
        assert!(matches!(c.lookup("b"), Lookup::Miss));
        assert!(matches!(c.lookup("c"), Lookup::Miss));
        assert!(matches!(c.lookup("d"), Lookup::Fresh(_)));
        assert!(matches!(c.lookup("e"), Lookup::Fresh(_)));
    }

    #[test]
    fn repo_prefix_of_paths() {
        assert_eq!(repo_prefix("/repos/o/n/pulls/3"), Some("/repos/o/n"));
        assert_eq!(repo_prefix("/repos/o/n"), Some("/repos/o/n"));
        assert_eq!(repo_prefix("/repos/o/n?x=1"), Some("/repos/o/n"));
        assert_eq!(repo_prefix("/user"), None);
        assert_eq!(repo_prefix("/repos/o"), None);
    }
}
