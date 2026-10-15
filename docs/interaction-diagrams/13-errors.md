# Errors and unknown URLs

```mermaid
sequenceDiagram
    participant B as Browser
    participant L as handle_errors (root layer)
    participant H as handler, or not_found!() catch-all
    B->>L: any request
    L->>H: next.run
    H-->>L: Err(AppError / NotFoundError / BadRequestError) or Ok
    alt Ok, or any other error (redirects, unknown faults)
        L-->>B: passed through untouched
    else known error, htmx request
        L-->>B: status (422 / 404 / 400 / 500) + HX-Reswap none + alert OOB to the page and the dialog
    else known error, navigation
        L-->>B: status + full branded error page
    end
```

| Error | Status | From |
|---|---|---|
| `AppError::Validation` | 422 | services: bad input, duplicates, delete guards, bad selection, bulk cap |
| `AppError::NotFound` | 404 | services: unknown id |
| `NotFoundError` | 404 | `not_found!()`: no route matched the URL |
| `BadRequestError` | 400 | `module_param!`: `{id}` isn't a number ("Invalid customer id.") |
| `AppError::Database` | 500 | anything else from Postgres; logged, shown as "Something went wrong" |

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
