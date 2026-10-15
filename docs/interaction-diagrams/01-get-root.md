# `GET /`

```mermaid
sequenceDiagram
    participant B as Browser
    participant H as app::home
    B->>H: GET /
    H-->>B: 303 See Other, Location /customers
    B->>B: follows to GET /customers (02-get-list-page.md)
```

---
[All endpoints](README.md) · [Per-resource differences](README.md#per-resource-differences) · [Errors](13-errors.md)
