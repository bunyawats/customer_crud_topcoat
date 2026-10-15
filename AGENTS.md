# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

# Topcoat + htmx + PostgreSQL Project Rules

A port of `customer_crud_web` (Axum + Askama) to [Topcoat](https://github.com/tokio-rs/topcoat). The
domain layers (models/repositories/services, migrations, `.sqlx/`) are carried over unchanged; the web
layer was rewritten as Topcoat module routes and `view!` components.

`.Codex/skills/topcoat-htmx-crud-stack` is this project's architecture playbook (layout, error
design, testing, UI patterns, and the Topcoat gotchas behind them). The user-global skills `htmx4`,
`tailwindcss4` and `rust-best-practices` in `~/.Codex/skills/` still apply. This file only states
what's specific to *this* project.

## Technology Stack
- **Framework:** Topcoat 0.10 (module routing, `view!` components, layouts/layers, assets). Needs Rust 1.98+.
- **Async runtime:** Tokio
- **Markup:** `view!` components in Rust - there is no `templates/` directory
- **Database:** SQLx + PostgreSQL (compile-time checked queries)
- **Interactivity:** htmx **v4** from `unpkg.com/htmx.org@next` (npm `next` = 4.0.0), plus `static/app.js`.
  Topcoat's `htmx` feature supplies `hx_request` and typed response headers (`HxReswap`, ...).
- **Styling:** Tailwind v4 via Topcoat's `tailwind` feature. `build.rs` compiles `static/input.css`
  (design tokens in `@theme`, `@source "../src"`), the output is bundled as an asset and linked with
  `tailwind::stylesheet!()`. Nothing generated is committed. Set `TAILWIND_CLI` to use a local binary
  instead of the auto-download.

Design tokens are unchanged from the original: cool paper/ink/muted/line neutrals, forest-green
`brand`, `danger` red, Fraunces display, IBM Plex Sans body, IBM Plex Mono for record data. Keep new
UI inside this token system. Shared class lists live as constants in `src/ui.rs` (`INPUT`, `SELECT`);
keep class names as string literals so Tailwind's scanner finds them.

## Development & Verification Commands
- Dev server with rebuild + asset bundling + live reload: `topcoat dev` (http://127.0.0.1:3000; `HOST`/`PORT` override)
- Check: `cargo check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
  - One integration file: `cargo test --test customer_routes` (also `product_routes`, `order_routes`, `app_routes`)
  - One test: `cargo test --test customer_routes duplicate_email` (name substring filter)
  - Unit tests only: `cargo test --lib`
- Format: `cargo fmt` and `topcoat fmt` (formats `view!` bodies)
- Run without the dev server: `topcoat asset bundle && cargo run`
- Migrations: `sqlx database create` (first time), then `sqlx migrate run`; after any query/schema change, `cargo sqlx prepare` and commit `.sqlx/`
- Dev sample data: `psql "$DATABASE_URL" -f seeds/dev.sql` (idempotent; kept out of `migrations/` so
  `#[sqlx::test]` databases start empty)

## Local development setup
- Copy `.env.example` to `.env` and set `DATABASE_URL`. `main.rs` loads it with `dotenvy`; `sqlx`
  macros read it at compile time. Without a reachable DB, build with `SQLX_OFFLINE=true`.
- `cargo install topcoat-cli` for `topcoat dev` / `asset bundle` / `fmt`.
- `main.rs` loads the asset bundle from next to the executable and panics with a hint if it's
  missing: run through `topcoat dev`, or `topcoat asset bundle` first.

---

## Test-Driven Generation (TDG) Protocol
1. **Mandatory tests.** Every new route, service function or repository query gets tests. Unit tests
   go in `#[cfg(test)] mod tests` at the bottom of the file. HTTP integration tests go in `/tests`,
   using `tests/common/mod.rs`'s `App` harness (real `Router::handle`, real layout and error layer).
   The harness bundles the test binary's own `asset!` declarations via the `topcoat-asset` dev-dep,
   so tests need no `topcoat asset bundle`. Use `htmx_get/post/put/delete` for fragment endpoints
   (sets `HX-Request`) and plain `get` for full-page navigations; `seed_customer`/`seed_product`
   and the `customer`/`product`/`order` form builders cover setup.
2. **Isolated databases.** Integration tests use `#[sqlx::test]`: a fresh, migrated, throwaway
   database per test (the connecting role needs `CREATEDB`). Never test against the dev database.
3. **Verification loop.** Not done until `cargo test` passes with zero failures (and clippy is clean).
4. **Regression safety.** If a change breaks a test, decide whether the code or the test's
   requirements are wrong before touching either.
5. Seed related rows through the real create routes and find their ids with
   `row_id_near(html, prefix, unique_text)`, never "first match" (see the playbook).

---

## Codebase Architecture
Two trees (rationale in the playbook):

```
src/
  lib.rs  main.rs  error.rs  ui.rs
  customer/ product/ order/      # domain: models.rs, repositories.rs, services.rs (no HTTP types)
  app.rs                         # "/": router(), root layout, error layer, not_found!(), "/" → /customers
  app/<resources>.rs             # "/<resources>": #[page] index, #[route(POST)] create, row/rows/form components
  app/<resources>/new.rs         # GET .../new          (create form fragment)
  app/<resources>/<x>_id.rs      # GET/PUT/DELETE .../{id} (row fragment, update, delete)
  app/<resources>/<x>_id/edit.rs # GET .../{id}/edit    (edit form fragment)
```

1. **Web tree (`src/app/`)** - extraction (`Form<T>`, `path_param`), handlers, view models
   (`CustomerRow`, `ProductRow`, `OrderRow`, `OrderFormData`), components. *Rule:* no SQL, no business
   logic. Pool via `app::pool(cx)`. Links via `href!`, not string paths.
2. **`services.rs`** - validation, business rules, cross-feature composition (e.g. `order::services`
   calling `customer::services::get` / `product::services::get`). *Rule:* no Topcoat types.
3. **`repositories.rs`** - SQLx only, `&PgPool` or `&mut Transaction<'_, Postgres>`.

`#[page]` handlers are wrapped by the root layout (`ui::shell`); `#[route]` handlers return bare
fragments for htmx. Keep that split: a fragment endpoint must not be a `#[page]`.

Component naming: `<resource>_row`, `<resource>_rows`, `<resource>_form`. Topcoat turns component
and handler names into unit structs, so a component named `form` or a page named `index` breaks local
bindings with those names.

---

## Error Handling Protocol (the unified AppError)
- Services and repositories return `AppError` (`src/error.rs`), never `sqlx::Error` or responses.
  Unique-constraint names map to friendly messages in `unique_violation_message` - add an arm for
  each new UNIQUE constraint.
- Handlers use `?`. The `handle_errors` layer in `src/app.rs` turns `AppError`, `NotFoundError` and
  `BadRequestError` into responses: htmx requests get the status, `HX-Reswap: none` and the
  `ui::alert` out-of-band fragment (both `#alert-region` and `#dialog-alert-region`); navigations get
  the full `ui::error_page`. Every other error passes through untouched - don't swallow redirects.

---

## PostgreSQL & SQLx Guardrails
- Use `sqlx::query!` / `query_as!` / `query_scalar!` for compile-time validation.
- Offline builds: `SQLX_OFFLINE=true`; regenerate `.sqlx/` with `cargo sqlx prepare` on query/schema changes.
- Multi-statement atomic writes take `&mut Transaction<'_, Postgres>`.

---

## Architectural Guardrails for Codex

### 1. Rust, lifetimes and the compiler-fix protocol
Follow the global `rust-best-practices` skill. Project specifics: the pool is Topcoat app context
(`.app_context(pool)` in `router()`, read with `app::pool(cx)`). Inside `view!`, don't pass `&` to a
temporary (`&format!(..)`, `&href!(..).resolve(cx)`) - bind it with `let` first.

### 2. htmx & UI separation
Follow the global `htmx4` skill. Project specifics: flatten domain data into view models in the web
tree; components receive view models, not domain entities; any JS reacting to htmx events uses v4
names (`htmx:after:swap`, `event.detail.ctx`). Post-submit dialog behaviour lives in `static/app.js`,
keyed by form id (`create-customer-form`, `edit-customer-form`, ...).
