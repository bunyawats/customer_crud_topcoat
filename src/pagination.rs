//! Page-at-a-time listing with a mint-once count cache (the
//! `list-pagination-bulk-actions` skill's pattern, ported from the
//! loan-onboarding-poc reference).
//!
//! The first request for a list runs the real `COUNT(*)`, caches the total
//! under a fresh `query_id` and returns it; later requests that echo that
//! `query_id` (Prev/Next) reuse the total and only run the row query. The
//! cache is a pure optimisation: an unknown, expired or mismatched `query_id`
//! recomputes, so it can never return a wrong answer, only a slightly stale
//! count (at most [`COUNT_CACHE_TTL`] old). Writes that change a list's size
//! (create, delete) just don't pass a `query_id`, forcing a fresh count.
//!
//! No Topcoat types here; the web layer maps form fields into a
//! [`PageRequest`] and renders the [`Page`].

use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::error::AppError;

/// Rows per page on every list screen.
pub const PAGE_SIZE: i64 = 10;

/// How long a cached total is trusted: long enough to cover a few Prev/Next
/// clicks, short enough that a count changed by another browser catches up.
pub const COUNT_CACHE_TTL: Duration = Duration::from_secs(30);

/// What the client asked for. `page` is 1-based and clamped into range;
/// `query_id` is the shortcut minted by an earlier response, if any.
#[derive(Debug, Clone, Default)]
pub struct PageRequest {
    pub page: i64,
    pub query_id: Option<String>,
}

impl PageRequest {
    pub fn first() -> Self {
        PageRequest {
            page: 1,
            query_id: None,
        }
    }

    /// Builds a request from raw form fields: an empty `query_id` means none.
    pub fn new(page: i64, query_id: &str) -> Self {
        PageRequest {
            page,
            query_id: (!query_id.is_empty()).then(|| query_id.to_string()),
        }
    }
}

/// One page of a list, plus what the pager needs to render itself.
#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    /// 1-based, already clamped to `1..=last_page()`.
    pub page: i64,
    pub page_size: i64,
    /// Echo this back on the next request for the same list.
    pub query_id: String,
}

impl<T> Page<T> {
    pub fn last_page(&self) -> i64 {
        last_page(self.total, self.page_size)
    }

    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    pub fn has_next(&self) -> bool {
        self.page < self.last_page()
    }

    /// 1-based number of the first row on this page, or 0 for an empty list.
    pub fn first_item_number(&self) -> i64 {
        if self.total == 0 {
            0
        } else {
            (self.page - 1) * self.page_size + 1
        }
    }

    /// 1-based number of the last row on this page, or 0 for an empty list.
    pub fn last_item_number(&self) -> i64 {
        (self.page * self.page_size).min(self.total)
    }

    /// Same page metadata, different items (e.g. domain rows to view models).
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            page_size: self.page_size,
            query_id: self.query_id,
        }
    }
}

/// The last valid page number: at least 1, so an empty list still has page 1.
pub fn last_page(total: i64, page_size: i64) -> i64 {
    ((total + page_size - 1) / page_size).max(1)
}

/// Clamps a requested page into `1..=last_page`, so a stale "page 4" after
/// deletes shows the new last page instead of an empty one.
pub fn clamp_page(page: i64, total: i64, page_size: i64) -> i64 {
    page.clamp(1, last_page(total, page_size))
}

/// The 1-based page holding the row at 0-based `position` in list order.
pub fn page_of_position(position: i64, page_size: i64) -> i64 {
    position.max(0) / page_size + 1
}

/// Runs one page of a list: the cached or fresh total, then the row query
/// for the clamped page. `filter_key` names the query's shape (e.g.
/// `"customers"`); a `query_id` minted for a different shape is ignored.
pub async fn fetch_page<T>(
    request: PageRequest,
    filter_key: &str,
    count: impl AsyncFnOnce() -> Result<i64, AppError>,
    list: impl AsyncFnOnce(i64, i64) -> Result<Vec<T>, AppError>,
) -> Result<Page<T>, AppError> {
    let cached = request
        .query_id
        .as_deref()
        .and_then(|id| COUNT_CACHE.lookup(id, filter_key, Instant::now()));
    let (total, query_id) = match (cached, request.query_id) {
        (Some(total), Some(query_id)) => (total, query_id),
        _ => {
            let total = count().await?;
            (total, COUNT_CACHE.store(filter_key, total, Instant::now()))
        }
    };

    let page = clamp_page(request.page, total, PAGE_SIZE);
    let items = list(PAGE_SIZE, (page - 1) * PAGE_SIZE).await?;
    Ok(Page {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        query_id,
    })
}

static COUNT_CACHE: LazyLock<CountCache> = LazyLock::new(|| CountCache::new(COUNT_CACHE_TTL));

struct CachedCount {
    filter_key: String,
    total: i64,
    expires_at: Instant,
}

/// In-process `query_id -> total` cache. Single-instance is fine: on another
/// replica (or after a restart) a `query_id` is simply a miss.
struct CountCache {
    ttl: Duration,
    entries: Mutex<HashMap<String, CachedCount>>,
}

impl CountCache {
    fn new(ttl: Duration) -> Self {
        CountCache {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn lookup(&self, query_id: &str, filter_key: &str, now: Instant) -> Option<i64> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.get(query_id)?;
        if now >= entry.expires_at {
            entries.remove(query_id);
            return None;
        }
        // A query_id is only a shortcut for "the same query as last time";
        // never reuse it for a different list's count.
        (entry.filter_key == filter_key).then_some(entry.total)
    }

    fn store(&self, filter_key: &str, total: i64, now: Instant) -> String {
        let query_id = mint_query_id();
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        // Prune on write so abandoned query_ids don't accumulate.
        entries.retain(|_, entry| now < entry.expires_at);
        entries.insert(
            query_id.clone(),
            CachedCount {
                filter_key: filter_key.to_string(),
                total,
                expires_at: now + self.ttl,
            },
        );
        query_id
    }
}

/// `q_` + 12 hex digits. Unique, not secret: guessing another query_id only
/// yields a cached total for a list this app shows everyone anyway, and a
/// different list's id is rejected by its filter_key.
fn mint_query_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static EPOCH: LazyLock<u64> = LazyLock::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    });
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("q_{:012x}", (*EPOCH << 20 | n) & 0xffff_ffff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_page_is_at_least_one() {
        assert_eq!(last_page(0, 10), 1);
        assert_eq!(last_page(1, 10), 1);
        assert_eq!(last_page(10, 10), 1);
        assert_eq!(last_page(11, 10), 2);
        assert_eq!(last_page(25, 10), 3);
    }

    #[test]
    fn requested_page_is_clamped_into_range() {
        assert_eq!(clamp_page(0, 25, 10), 1);
        assert_eq!(clamp_page(-3, 25, 10), 1);
        assert_eq!(clamp_page(2, 25, 10), 2);
        assert_eq!(clamp_page(9, 25, 10), 3);
        assert_eq!(clamp_page(2, 0, 10), 1);
    }

    #[test]
    fn position_maps_to_its_page() {
        assert_eq!(page_of_position(0, 10), 1);
        assert_eq!(page_of_position(9, 10), 1);
        assert_eq!(page_of_position(10, 10), 2);
        assert_eq!(page_of_position(24, 10), 3);
    }

    fn page(total: i64, page: i64) -> Page<()> {
        Page {
            items: Vec::new(),
            total,
            page,
            page_size: 10,
            query_id: String::new(),
        }
    }

    #[test]
    fn pager_numbers_and_bounds() {
        let middle = page(25, 2);
        assert_eq!(
            (middle.first_item_number(), middle.last_item_number()),
            (11, 20)
        );
        assert!(middle.has_prev() && middle.has_next());

        let last = page(25, 3);
        assert_eq!(
            (last.first_item_number(), last.last_item_number()),
            (21, 25)
        );
        assert!(last.has_prev() && !last.has_next());

        let empty = page(0, 1);
        assert_eq!(
            (empty.first_item_number(), empty.last_item_number()),
            (0, 0)
        );
        assert!(!empty.has_prev() && !empty.has_next());
    }

    #[test]
    fn empty_query_id_means_none() {
        assert_eq!(PageRequest::new(2, "").query_id, None);
        assert_eq!(PageRequest::new(2, "q_1").query_id.as_deref(), Some("q_1"));
    }

    #[test]
    fn cached_total_is_reused_until_it_expires() {
        let cache = CountCache::new(Duration::from_secs(30));
        let t0 = Instant::now();
        let id = cache.store("customers", 42, t0);

        assert_eq!(
            cache.lookup(&id, "customers", t0 + Duration::from_secs(29)),
            Some(42)
        );
        assert_eq!(
            cache.lookup(&id, "customers", t0 + Duration::from_secs(30)),
            None
        );
        // Expired entries are dropped, so a later lookup is still a miss.
        assert_eq!(cache.lookup(&id, "customers", t0), None);
    }

    #[test]
    fn query_id_for_another_list_is_ignored() {
        let cache = CountCache::new(Duration::from_secs(30));
        let t0 = Instant::now();
        let id = cache.store("customers", 42, t0);
        assert_eq!(cache.lookup(&id, "products", t0), None);
        assert_eq!(cache.lookup("q_unknown", "customers", t0), None);
    }

    #[test]
    fn minted_ids_are_unique() {
        let a = mint_query_id();
        let b = mint_query_id();
        assert_ne!(a, b);
        assert!(a.starts_with("q_") && a.len() == 14);
    }

    #[tokio::test]
    async fn fetch_page_counts_once_then_reuses_the_query_id() {
        use std::sync::atomic::AtomicUsize;
        let counts = AtomicUsize::new(0);
        let count = async || {
            counts.fetch_add(1, Ordering::SeqCst);
            Ok(25)
        };
        let list = async |limit: i64, offset: i64| Ok(vec![(limit, offset)]);

        let first = fetch_page(PageRequest::first(), "test-list", count, list)
            .await
            .unwrap();
        assert_eq!(first.items, vec![(10, 0)]);
        assert_eq!(first.total, 25);

        let next = fetch_page(
            PageRequest::new(3, &first.query_id),
            "test-list",
            async || {
                counts.fetch_add(1, Ordering::SeqCst);
                Ok(999)
            },
            list,
        )
        .await
        .unwrap();
        assert_eq!(next.total, 25, "cached total reused");
        assert_eq!(next.query_id, first.query_id);
        assert_eq!(next.items, vec![(10, 20)]);
        assert_eq!(counts.load(Ordering::SeqCst), 1, "COUNT ran once");
    }

    #[tokio::test]
    async fn fetch_page_clamps_past_the_end() {
        let page = fetch_page(
            PageRequest::new(9, ""),
            "test-clamp",
            async || Ok(25),
            async |limit: i64, offset: i64| Ok(vec![(limit, offset)]),
        )
        .await
        .unwrap();
        assert_eq!(page.page, 3);
        assert_eq!(page.items, vec![(10, 20)]);
    }
}
