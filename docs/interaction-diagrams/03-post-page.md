# `POST /<res>/page`: Prev / Next

```mermaid
sequenceDiagram
    participant B as Browser (htmx)
    participant H as res::page::show_page
    participant S as res::services
    participant R as res::repositories
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant DB as PostgreSQL
    B->>H: POST /customers/page (page, query_id from the button's hx-vals)
    H->>S: list_page(page, query_id)
    alt query_id cached, not expired, same list
        S->>S: reuse the cached total (no COUNT)
    else unknown, expired or missing
        S->>R: count
        R->>DB: SELECT COUNT(*)
        S->>S: mint a new query_id
    end
    S->>S: clamp page into 1..last page
    S->>R: list_page(10, (page-1)*10)
    R->>DB: SELECT ... LIMIT 10 OFFSET ...
    S-->>H: Page
    H->>SelS: get(session) via app::selected_ids (skipped without a session)
    SelS->>SelR: get
    SelR->>DB: SELECT item_id FROM selected_items
    H-->>B: list region (rows with checkbox states, pager, new list state)
    B->>B: swap the list region (outerHTML)
```

Paging never changes the selection; ticked rows on other pages stay ticked.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
