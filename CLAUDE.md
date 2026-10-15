# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Topcoat + htmx + PostgreSQL Project Rules

A port of `customer_crud_web` (Axum + Askama) to [Topcoat](https://github.com/tokio-rs/topcoat). The
domain layers (models/repositories/services, migrations, `.sqlx/`) are carried over unchanged; the web
layer was rewritten as Topcoat module routes and `view!` components.

`.claude/skills/topcoat-htmx-crud-stack` is this project's architecture playbook (layout, error
design, testing, UI patterns, and the Topcoat gotchas behind them). The user-global skills `htmx4`,
`tailwindcss4` and `rust-best-practices` in `~/.claude/skills/` still apply. This file only states
what's specific to *this* project.

## Lists: pagination, selection, bulk delete
All three lists page 10 rows at a time, keep a server-side multi-select, and bulk delete. The
decisions, as-built notes and gotchas live in
**[`docs/PAGINATION_BULK_ACTIONS_PLAN.md`](docs/PAGINATION_BULK_ACTIONS_PLAN.md)**; read its
"As built" notes before changing a list page. The essentials:
- **Regions per list page:** `#<res>-toolbar` (selected count, "Delete selected") and `#<res>-list`
  (hidden `#<res>-list-state` inputs `page`/`query_id`, the table, the pager). Every list-changing
  response replaces `#<res>-list` whole and sends the toolbar out-of-band.
- **Counts:** `src/pagination.rs` caches `COUNT(*)` for 30 s under a `query_id` that Prev/Next echo
  back. Writes (create, delete) re-render without a `query_id`, forcing a fresh count. List queries
  need an `id` tiebreaker in `ORDER BY` so `OFFSET` paging is stable.
- **Selection:** stored in `selected_items` (`src/selection/`), keyed by the anonymous Topcoat
  session's token hash (`app::session_key` / `ensure_session_key`; only the selection route starts
  a session). A list `#[page]` visit clears it; Prev/Next never do. Bulk routes re-read it
  server-side and never trust ids from the client.
- **Bulk delete:** `src/bulk.rs` fans out over the resource's own `services::delete`, so its guards
  report per item; at most `bulk::MAX_BATCH` (100) ids.
- htmx v4 sends `DELETE` parameters in the **query string**: read them with `#[query_params]`
  (`app::page_after_write`), not `Form<T>`.

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
- Dev server with rebuild + asset bundling + live reload: `PORT=3010 topcoat dev`
  (http://127.0.0.1:3010). This project runs on **3010**; 3000 is taken by the sibling
  `customer_crud_web` app.
  - `topcoat dev` reads only the shell's `PORT` (default 3000) and passes it to the app, so `PORT`
    in `.env` is ignored (`dotenvy` doesn't override variables already set) - always prefix it.
    If the port is busy it falls back to the next one (`port 3010 is in use; starting on port
    3011 instead`), so check the `ready on` line for the real URL.
  - Plain `cargo run` does read `HOST`/`PORT` from `.env`.
- Check: `cargo check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
  - One integration file: `cargo test --test customer_routes` (also `product_routes`, `order_routes`, `app_routes`)
  - One test: `cargo test --test customer_routes duplicate_email` (name substring filter)
  - Unit tests only: `cargo test --lib`
- Format: `cargo fmt` and `topcoat fmt` (formats `view!` bodies)
- Run without the dev server: `topcoat asset bundle && cargo run`
- Migrations: `sqlx database create` (first time), then `sqlx migrate run`; after any query/schema change, `cargo sqlx prepare` and commit `.sqlx/`
- Schema: [`docs/er-diagram.md`](docs/er-diagram.md) (ER diagram, delete rules, money rules). Update it in
  the same change as any new migration.
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
Diagrams (runtime, build tooling, module dependencies):
[`docs/architecture.md`](docs/architecture.md). Every endpoint, one sequence diagram per file:
[`docs/interaction-diagrams/`](docs/interaction-diagrams/README.md); add a file there for each new
route. Two trees (rationale in the playbook):

```
src/
  lib.rs  main.rs  error.rs  ui.rs
  customer/ product/ order/      # domain: models.rs, repositories.rs, services.rs (no HTTP types)
  app.rs                         # "/": router(), root layout, error layer, not_found!(), "/" → /customers
  app/<resources>.rs             # "/<resources>": #[page] index, #[route(POST)] create, row/rows/form components
  app/<resources>/new.rs         # GET .../new          (create form fragment)
  app/<resources>/<x>_id.rs      # GET/PUT/DELETE .../{id} (row fragment, update, delete)
  app/<resources>/<x>_id/edit.rs # GET .../{id}/edit    (edit form fragment)
  app/<resources>/page.rs        # POST .../page        (Prev/Next: list region)
  app/<resources>/selection.rs   # POST .../selection   (checkbox / select-all)
  app/<resources>/bulk_delete.rs # POST .../bulk-delete (+ bulk_delete/confirm.rs: the confirm dialog)
  pagination.rs  bulk.rs         # shared domain helpers: Page/count cache; batch prep + fan-out
  selection/                     # domain: server-side selection store (no HTTP types)
```

1. **Web tree (`src/app/`)** - extraction (`Form<T>`, `path_param`), handlers, view models
   (`CustomerRow`, `ProductRow`, `OrderRow`, `OrderFormData`), components. *Rule:* no SQL, no business
   logic. Pool via `app::pool(cx)`. Links via `href!`, not string paths.
2. **`services.rs`** - validation, business rules, cross-feature composition (e.g. `order::services`
   calling `customer::services::get` / `product::services::get`). *Rule:* no Topcoat types.
3. **`repositories.rs`** - SQLx only, `&PgPool` or `&mut Transaction<'_, Postgres>`.

`#[page]` handlers are wrapped by the root layout (`ui::shell`); `#[route]` handlers return bare
fragments for htmx. Keep that split: a fragment endpoint must not be a `#[page]`.

Component naming: `<resource>_row`, `<resource>_rows`, `<resource>_form`, `<resource>_list`,
`<resource>_toolbar`. Topcoat turns component and handler names into unit structs, so a component
named `form` or a page named `index` breaks local bindings with those names (hit again here: a local
`let list_state` next to the `ui::list_state` component).

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

## Architectural Guardrails for Claude Code

### 1. Rust, lifetimes and the compiler-fix protocol
Follow the global `rust-best-practices` skill. Project specifics: the pool is Topcoat app context
(`.app_context(pool)` in `router()`, read with `app::pool(cx)`). Inside `view!`, don't pass `&` to a
temporary (`&format!(..)`, `&href!(..).resolve(cx)`) - bind it with `let` first.

### 2. htmx & UI separation
Follow the global `htmx4` skill. Project specifics: flatten domain data into view models in the web
tree; components receive view models, not domain entities; any JS reacting to htmx events uses v4
names (`htmx:after:swap`, `event.detail.ctx`). Post-submit dialog behaviour lives in `static/app.js`,
keyed by form id (`create-customer-form`, `edit-customer-form`, ...).
