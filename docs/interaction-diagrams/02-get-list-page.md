# `GET /<res>`: the list page

```mermaid
sequenceDiagram
    participant B as Browser
    participant H as res::index
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant S as res::services
    participant R as res::repositories
    participant DB as PostgreSQL
    B->>H: GET /customers (a navigation, not htmx)
    opt the browser already has a session cookie
        H->>SelS: clear(session, Customers) via app::clear_selection
        SelS->>SelR: clear
        SelR->>DB: DELETE FROM selected_items (this session, this resource)
    end
    H->>S: list_page(page 1, no query_id)
    S->>S: pagination::fetch_page: cache miss
    S->>R: count
    R->>DB: SELECT COUNT(*)
    S->>S: mint query_id, cache the total for 30 s
    S->>R: list_page(10, 0)
    R->>DB: SELECT ... ORDER BY ... LIMIT 10 OFFSET 0
    R-->>S: rows
    S-->>H: Page (rows, total, query_id)
    H-->>B: 200 page: shell + header + toolbar (0 selected) + list region
```

A fresh visit always starts with nothing selected. Viewing a page never starts a session.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
