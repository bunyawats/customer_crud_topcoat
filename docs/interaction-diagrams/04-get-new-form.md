# `GET /<res>/new`: open the create form

```mermaid
sequenceDiagram
    participant B as Browser (htmx + app.js)
    participant H as res::new::new_form
    participant S as customer / product services
    participant R as customer / product repositories
    participant DB as PostgreSQL
    B->>H: GET /customers/new ("Add customer" button)
    opt orders only (OrderFormData::load)
        H->>S: customer::services::list, product::services::list
        S->>R: list
        R->>DB: SELECT customers, SELECT products
    end
    H-->>B: empty create form (posts to /customers, swaps the list region)
    B->>B: swap into the dialog content, app.js opens the dialog
```

Submitting it is [05-post-create.md](05-post-create.md).

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
