# `DELETE /<res>/{id}`: delete one row

```mermaid
sequenceDiagram
    actor U as User
    participant B as Browser (htmx)
    participant H as res::x_id::delete
    participant S as res::services
    participant OS as order::services
    participant R as repositories
    participant FS as filter::services
    participant SelS as selection::services
    participant SelR as selection::repositories
    participant DB as PostgreSQL
    U->>B: clicks the row's trash icon
    B->>U: native confirm ("Delete this customer?")
    U->>B: OK
    B->>H: DELETE /customers/7?page=3 (page from the list state, in the URL)
    H->>S: delete(7)
    opt customers and products: the guard
        S->>OS: count_for_customer(7) / count_for_product(7)
        OS->>R: order repo::count_for_customer / count_for_product
        R->>DB: SELECT COUNT(*) FROM orders / order_items
        Note over S,OS: count > 0: AppError::Validation, 422 alert, nothing deleted
    end
    S->>R: res repo::delete
    R->>DB: DELETE (orders: items cascade)
    R-->>S: deleted? (no row: AppError::NotFound)
    H->>SelS: forget_item(Customers, 7)
    SelS->>SelR: forget_item
    SelR->>DB: DELETE FROM selected_items (every session)
    H->>FS: get(session) via app::list_filter
    H->>S: list_page(page 3 from #[query_params], fresh count, clamped, filter)
    S->>R: count, list_page
    R->>DB: SELECT COUNT(*), SELECT ... LIMIT 10 OFFSET 20
    H->>SelS: get(session) via app::selected_ids
    SelS->>SelR: get
    H-->>B: list region + toolbar (OOB)
    B->>B: swap the list region, the page refills from the next one
```

htmx v4 sends a `DELETE`'s parameters in the query string, so the handler reads `page` with
`#[query_params]`.

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
