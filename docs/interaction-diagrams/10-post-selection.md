# `POST /<res>/selection`: tick a row or a page

```mermaid
sequenceDiagram
    participant B as Browser (htmx)
    participant H as res::selection::update_selection
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant S as res::services
    participant R as res::repositories
    participant DB as PostgreSQL
    B->>H: POST /customers/selection (ids "7" or "1,2,...,10", checked, list state)
    H->>SelS: parse_ids (non-numeric: 422 alert)
    opt no session yet (first tick)
        H->>H: ensure_session_key: session::start sets the __Host-session cookie
    end
    H->>SelS: update(session, Customers, ids, checked) via app::apply_selection
    SelS->>SelR: purge_older_than(1 day)
    SelR->>DB: DELETE old selected_items
    SelS->>SelR: add(ids) or remove(ids)
    SelR->>DB: INSERT ... ON CONFLICT DO NOTHING / DELETE ... = ANY(ids)
    SelS->>SelR: get
    SelR->>DB: SELECT item_id FROM selected_items
    SelS-->>H: selection after the change
    H->>S: list_page(included page, query_id)
    S->>R: list_page (count only on a cache miss)
    R->>DB: SELECT ... LIMIT 10 OFFSET ...
    H-->>B: list region + toolbar (OOB) with the new count
    alt row checkbox (hx-swap none)
        B->>B: keeps its own box, applies only the toolbar
    else select-all checkbox
        B->>B: swaps the list region, every row box follows
    end
```

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
