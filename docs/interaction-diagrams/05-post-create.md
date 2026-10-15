# `POST /<res>`: create

```mermaid
sequenceDiagram
    participant B as Browser (htmx + app.js)
    participant L as handle_errors
    participant H as res::create
    participant S as res::services
    participant O as customer / product services
    participant FS as filter::services
    participant R as repositories
    participant DB as PostgreSQL
    B->>L: POST /customers (form fields, orders send items_json)
    L->>H: next.run
    H->>S: create(form)
    S->>S: validate
    opt orders: resolve
        S->>O: customer::services::get, product::services::get per item
        O->>R: customer / product repo::find
        R->>DB: SELECT
        O-->>S: rows (missing: "Selected customer no longer exists.")
        S->>S: snapshot each product's price
    end
    S->>R: res repo::create
    R->>DB: INSERT (orders: header + items in one transaction)
    alt valid
        R-->>S: new row / id
        H->>S: page_of(new id)
        S->>R: position_of
        R->>DB: COUNT rows sorting before it
        H->>FS: clear(session) via app::clear_filter (so the new row can't be filtered out)
        FS->>DB: DELETE FROM list_filters
        H->>S: list_page(that page, no query_id, no filter)
        S->>R: count, then list_page
        R->>DB: SELECT COUNT(*), SELECT ... LIMIT 10 OFFSET ...
        H-->>B: list region on the page holding the new row
        B->>B: swap the list region, app.js closes the dialog
    else invalid (blank field, duplicate email or SKU, unknown product ...)
        Note over S,R: a duplicate is a unique violation from INSERT, mapped to a message in error.rs
        S-->>H: AppError::Validation
        H-->>L: Err
        L-->>B: 422 + HX-Reswap none + alert fragment (OOB)
        B->>B: dialog stays open with the alert inside it
    end
```

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
