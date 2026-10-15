# customer_crud_topcoat

A small customer / product / order ledger built with [Topcoat](https://github.com/tokio-rs/topcoat),
htmx 4, Tailwind v4 and SQLx + PostgreSQL. It's a port of
[customer_crud_web](https://github.com/bunyawats/customer_crud_web) (Axum + Askama) with the same
schema and look, extended with paged lists and bulk delete.

- Create, edit and delete customers, products and orders in a modal dialog, without page reloads
- Orders have any number of line items; totals use prices snapshotted server-side
- Every list pages 10 rows at a time (Prev/Next, with the row count cached briefly instead of
  re-counted on every click)
- Tick rows on any page, or a whole page at once; the selection is kept server-side, so it
  survives paging. "Delete selected" shows what will go, deletes what it can, and lists anything
  it couldn't (e.g. a customer who still has orders)
- Friendly validation errors (duplicate email/SKU, deleting a customer who still has orders, ...)
  shown as an auto-dismissing alert, without disturbing the page

## Requirements

- Rust **1.98+** (Topcoat 0.10's minimum)
- PostgreSQL
- The Topcoat CLI: `cargo install topcoat-cli`
- `sqlx-cli` for migrations: `cargo install sqlx-cli --no-default-features --features postgres,rustls`

Tailwind's standalone CLI is downloaded automatically by the build. No Node.js is needed.

## Run it

```sh
cp .env.example .env          # then set DATABASE_URL
sqlx database create
sqlx migrate run
psql "$DATABASE_URL" -f seeds/dev.sql   # optional sample data (safe to re-run)
PORT=3010 topcoat dev         # builds, bundles assets, serves http://127.0.0.1:3010, reloads on change
```

Without the dev server: `topcoat asset bundle && cargo run`.

## Test

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Integration tests use `#[sqlx::test]`, which creates a throwaway database per test, so the role in
`DATABASE_URL` needs `CREATEDB`.

## How it's organized

```
src/customer, src/product, src/order   domain: models → repositories → services (no HTTP)
src/app.rs, src/app/…                  web: Topcoat module routes mirroring the URLs
src/ui.rs                              shared view! components (layout shell, fields, icons, alert)
src/error.rs                           AppError, rendered by one error layer in src/app.rs
static/input.css, static/app.js        Tailwind source and the small dialog/line-item script
```

`CLAUDE.md` and `.claude/skills/topcoat-htmx-crud-stack/` describe the architecture and conventions
in detail.

### What changed from the Axum version

| customer_crud_web | customer_crud_topcoat |
|---|---|
| Axum router listing every route in `app/mod.rs` | `module_router!()`: URLs derived from `src/app/` modules |
| 19 Askama templates + a `#[derive(Template)]` struct each | `view!` components next to their routes |
| `is_htmx` + `respond` + `AppError::into_response(is_htmx)` in every handler | Handlers use `?`; one root `#[layer]` renders errors |
| `HX-Reswap` set by hand | `HxReswap(SwapOption::None)` from `topcoat::htmx` |
| Tailwind binary downloaded by hand, compiled CSS committed | `build.rs` + `tailwind` feature; nothing generated is committed |
| `tower-http` `ServeDir` for `/static` | `asset!` with content-hashed, cache-forever URLs |
| Bare 404 for unknown URLs | Branded 404 via `not_found!()` |

Also fixed on the way: the customer forms' Cancel button called an undefined `customerDialog`.
