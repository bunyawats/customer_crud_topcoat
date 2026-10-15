# `POST /<res>/search`: run or clear a search

The only request that carries search criteria. It stores them as this browser's filter for the
list, and every other list request ([2](02-get-list-page.md), [3](03-post-page.md),
[8](08-delete-row.md), [10](10-post-selection.md)–[12](12-post-bulk-delete.md)) reads them back
from the server.

```mermaid
sequenceDiagram
    participant B as Browser (htmx)
    participant H as res::search::run_search
    participant FS as filter::services
    participant FR as filter::repositories
    participant S as res::services
    participant R as res::repositories
    participant SelS as selection::services
    participant DB as PostgreSQL
    B->>H: POST /customers/search (q from the search form, or q="" from Clear)
    H->>H: Search::new(q) - trim, collapse spaces, max 100 chars
    alt q longer than 100 characters
        H-->>B: 422 alert (via handle_errors), stored filter unchanged
    else q is empty (Clear)
        H->>FS: clear (only if a session exists, never starts one)
        FS->>FR: clear
        FR->>DB: DELETE FROM list_filters
    else a term
        H->>H: app::ensure_session_key (starts a session on first use)
        H->>FS: set(session, resource, ListFilter)
        FS->>FR: purge_older_than(1 day), then upsert
        FR->>DB: INSERT INTO list_filters ... ON CONFLICT DO UPDATE
    end
    H->>S: list_page(page 1, no query_id, filter)
    S->>R: count(pattern)
    R->>DB: SELECT COUNT(*) ... WHERE ... ILIKE pattern
    S->>R: list_page(pattern, 10, 0)
    R->>DB: SELECT ... WHERE ... ILIKE pattern LIMIT 10
    S-->>H: Page (fresh query_id keyed by the filter)
    H->>SelS: get(session) via app::selected_ids
    SelS->>DB: SELECT item_id FROM selected_items
    H-->>B: list region, plus the search box OOB (emptied) when clearing
    B->>B: swap the list region (outerHTML)
```

- **Ticks are kept**: a search never touches `selected_items`, so rows ticked under another
  search stay ticked and still count in the toolbar.
- The pattern is `%term%` with `\`, `%` and `_` escaped, so they match literally. What each list
  matches is in [Per-resource differences](README.md#per-resource-differences).
- The count cache key includes the filter, so a `query_id` minted before the search is a miss.
- A normal search doesn't re-render the box, so the input keeps its text and focus.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
