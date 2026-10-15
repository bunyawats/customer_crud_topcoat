# `POST /<res>/page`: Prev / Next

```mermaid
sequenceDiagram
    participant B as Browser (htmx)
    participant H as res::page::show_page
    participant FS as filter::services
    participant S as res::services
    participant R as res::repositories
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant DB as PostgreSQL
    B->>H: POST /customers/page (page, query_id from the button's hx-vals)
    H->>FS: get(session) via app::list_filter (no session: no filter)
    FS->>DB: SELECT criteria FROM list_filters
    H->>S: list_page(page, query_id, filter)
    alt query_id cached, not expired, same list and same filter
        S->>S: reuse the cached total (no COUNT)
    else unknown, expired or missing
        S->>R: count
        R->>DB: SELECT COUNT(*)
        S->>S: mint a new query_id
    end
    S->>S: clamp page into 1..last page
    S->>R: list_page(pattern, 10, (page-1)*10)
    R->>DB: SELECT ... LIMIT 10 OFFSET ...
    S-->>H: Page
    H->>SelS: get(session) via app::selected_ids (skipped without a session)
    SelS->>SelR: get
    SelR->>DB: SELECT item_id FROM selected_items
    H-->>B: list region (rows with checkbox states, pager, new list state)
    B->>B: swap the list region (outerHTML)
```

Paging never changes the selection; ticked rows on other pages stay ticked. Any criteria in the
posted form are ignored: the filter comes only from the server.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
