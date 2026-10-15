# `GET /<res>/{id}/edit`: open the edit form

```mermaid
sequenceDiagram
    participant B as Browser (htmx + app.js)
    participant H as res::x_id::edit::edit_form
    participant S as res::services
    participant R as repositories
    participant DB as PostgreSQL
    B->>H: GET /customers/7/edit (row Edit icon)
    H->>S: get(7)
    S->>R: repo::find (orders: find + find_items)
    R->>DB: SELECT the row (and its line items)
    alt found
        opt orders only (OrderFormData::load)
            H->>S: customer::services::list, product::services::list
            S->>R: list
            R->>DB: SELECT customers, SELECT products
        end
        H-->>B: form pre-filled (puts to /customers/7, swaps that row)
        B->>B: swap into the dialog content, app.js opens the dialog
    else unknown id
        S-->>H: AppError::NotFound
        H-->>B: 404 + HX-Reswap none + "Customer not found." alert
    end
```

Submitting it is [07-put-update.md](07-put-update.md).

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
