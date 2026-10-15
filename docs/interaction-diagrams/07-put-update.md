# `PUT /<res>/{id}`: update

```mermaid
sequenceDiagram
    participant B as Browser (htmx + app.js)
    participant H as res::x_id::update
    participant S as res::services
    participant R as repositories
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant DB as PostgreSQL
    B->>H: PUT /customers/7 (form fields)
    H->>S: update(7, form)
    S->>S: validate (orders: resolve customer, products, prices as in create)
    S->>R: repo::update
    R->>DB: UPDATE (orders: header, delete + re-insert items, one transaction)
    alt row existed
        R-->>S: updated row (orders: then get = find + find_items)
        S-->>H: updated row
        H->>SelS: get(session) via app::selected_ids
        SelS->>SelR: get
        SelR->>DB: SELECT item_id FROM selected_items
        H-->>B: one row fragment, checkbox state kept
        B->>B: swap that row (outerHTML), app.js closes the dialog
    else no such id
        S-->>H: AppError::NotFound, 404 alert
    end
    Note over B,H: A validation error is a 422 alert and the dialog stays open.
```

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
