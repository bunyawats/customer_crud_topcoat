# `POST /<res>/bulk-delete/confirm`: the confirm dialog

```mermaid
sequenceDiagram
    participant B as Browser (htmx + app.js)
    participant H as res::bulk_delete::confirm
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant S as res::services
    participant R as res::repositories
    participant DB as PostgreSQL
    B->>H: POST /customers/bulk-delete/confirm (page from the list state)
    H->>SelS: get(session) (nothing without a session)
    SelS->>SelR: get
    SelR->>DB: SELECT item_id FROM selected_items
    H->>H: bulk::prepare_batch: empty, or over 100 ids
    opt within the cap
        loop each selected id (bulk_items)
            H->>S: get(id) for a fresh label
            S->>R: repo::find
            R->>DB: SELECT the row
            Note over H,S: NotFound (deleted elsewhere): dropped from the list
        end
    end
    H-->>B: confirm dialog: the rows, or "Nothing is selected", or the over-cap error
    B->>B: swap into the dialog content, app.js opens the dialog
```

The dialog's form carries only `page`; it never sends ids.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
