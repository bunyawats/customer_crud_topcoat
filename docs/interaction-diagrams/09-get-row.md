# `GET /<res>/{id}`: one row

```mermaid
sequenceDiagram
    participant C as Client
    participant H as res::x_id::show_row
    participant S as res::services
    participant R as repositories
    participant SelS as selection::services
    participant DB as PostgreSQL
    C->>H: GET /customers/7
    H->>S: get(7)
    S->>R: repo::find (orders: find + find_items)
    R->>DB: SELECT the row
    H->>SelS: get(session) via app::selected_ids
    H-->>C: one row fragment (404 alert if missing, 400 if the id isn't a number)
```

Nothing in the UI calls this today; the integration tests use it to check 404 and 400 handling. It
is the natural target for a "cancel edit, re-render the row" action if one is ever added.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
