---
name: topcoat-htmx-crud-stack
description: Playbook for building a server-rendered CRUD app the way this project does - Topcoat (module routing, view! components, layouts, layers, assets, built-in Tailwind) + htmx + SQLx/Postgres. Covers the two-tree layout (feature-first domain modules + module-routed web tree), the AppError-through-a-root-layer error design, testing through Router::handle with sqlx::test, the dialog-based CRUD UI with dynamic line-item rows, and the Topcoat 0.10 gotchas hit while porting from Axum + Askama. Use when adding a feature to this project or starting a new one with the same architecture.
---

# Topcoat + htmx + SQLx CRUD Stack

The concrete playbook behind `customer_crud_topcoat`, a port of `customer_crud_web` (Axum + Askama).
It composes with the user-global skills rather than duplicating them: **`htmx4`** (htmx conventions,
v4 event names), **`rust-best-practices`** (general Rust idioms) and **`tailwindcss4`** (deriving a
visual identity for a *new* subject). Starting a new project from this playbook: copy this skill into
the new repo's `.Codex/skills/` and confirm those three exist in `~/.Codex/skills/`.

## The one meta-lesson: verify, don't remember

Topcoat is young (0.x, "expect breaking changes"), so memorized API knowledge goes stale fast. Read
the docs for the version you depend on (`docs.rs/topcoat/<version>`, or the crate source in
`~/.cargo/registry/src/*/topcoat-*`), and its `examples/` directory, before writing against an API.
Things verified while building this project, each of which a guess would have gotten wrong:

- **Topcoat 0.10 needs Rust 1.98** - it uses `int_format_into`, stabilized in 1.98. On an older
  toolchain every Topcoat crate fails with E0658. `rustup update stable`.
- **`#[component]`, `#[page]` and `#[route]` define a unit struct named after the function.** A
  component called `form` breaks `Form(form): Form<T>` ("`form` is interpreted as a unit struct"), and
  a page called `index` breaks `for (index, item) in ...`. Name components `<resource>_form`,
  `<resource>_row`, and don't reuse handler names as local bindings.
- **No temporaries as `&` component arguments inside `view!`**: `edit_url: &href!(..).resolve(cx)` or
  `label: &format!(..)` fails with E0716, because views are lazy and outlive the statement. Bind with
  `let` before `view!` and pass `&binding`.
- **Rendering an `Asset` missing from the loaded bundle panics.** Pages render `asset!` URLs (the
  Tailwind stylesheet, app.js), so the app needs a bundle at runtime (`topcoat dev` /
  `topcoat asset bundle`) *and in tests* - see Testing.
- **Path layers don't run for unrouted URLs.** A 404 for a URL nothing matched skips `#[layer]`s and
  layouts unless the root module declares `not_found!()`, which turns them into a `NotFoundError`
  that flows through the error layer like any other.
- **Topcoat's Tailwind downloader uses its own certificate store**, so it fails behind TLS-intercepting
  proxies. `build.rs` honours `TAILWIND_CLI=/path/to/tailwindcss` to use a local binary instead.
- htmx: npm's `next` dist-tag is **4.0.0** while `latest` is still 2.x (checked with
  `npm view htmx.org dist-tags`), so `unpkg.com/htmx.org@next` is v4. Re-check before pinning.
- Carried over from the Axum project and still true: `serde_urlencoded` (what `Form<T>` uses) does
  not zip repeated same-named fields into aligned `Vec`s; it errors. Hence the `items_json` pattern.

## Stack

- **Framework:** Topcoat 0.10 with `default-features = false` and
  `features = ["router", "serve", "discover", "asset", "htmx", "tailwind"]`
- **Runtime:** Tokio (`rt-multi-thread`, `macros`)
- **Markup:** `view!` components - no template files, no template engine
- **Database:** SQLx + PostgreSQL (`postgres`, `runtime-tokio`, `tls-rustls`, `chrono`)
- **Interactivity:** htmx 4 from the CDN (see the global `htmx4` skill), plus a small `static/app.js`
- **Styling:** Tailwind v4 through Topcoat's `tailwind` feature (a `build.rs` runs the standalone
  CLI, so there is no Node.js, no committed binary and no committed CSS)
- **Dev tooling:** `topcoat-cli` (`cargo install topcoat-cli`: `topcoat dev`, `topcoat asset bundle`,
  `topcoat fmt`), `sqlx-cli`

## Project layout: two trees

```
src/
  lib.rs                 # declares modules, re-exports router()
  main.rs                # thin: env, pool, AssetBundle::load(), topcoat::start(router(..))
  error.rs               # AppError (domain error, implements std::error::Error)
  ui.rs                  # shared view! components: shell, icons, fields, buttons, alert
  customer/ product/ order/      # DOMAIN tree - feature-first, no HTTP types
    models.rs  repositories.rs  services.rs
  app.rs                 # WEB tree root = "/": router(), layout, error layer, not_found!()
  app/
    customers.rs         # /customers: index page, POST create, row/rows/form components
    customers/new.rs     # /customers/new
    customers/customer_id.rs        # /customers/{customer_id}: GET row, PUT, DELETE
    customers/customer_id/edit.rs   # /customers/{customer_id}/edit
    products...  orders...          # same shape
static/
  input.css              # Tailwind source; @source "../src" (markup is in Rust now)
  app.js                 # dialog lifecycle, line-item rows, alert auto-dismiss
build.rs                 # Tailwind via Topcoat
tests/
  common/mod.rs          # App harness: Router::handle + bundled test assets + seed helpers
  <resource>_routes.rs   # full-HTTP integration tests
  app_routes.rs          # layout / error layer / 404 behaviour
```

**Why two trees.** `module_router!()` derives URLs from module paths, so the web tree has to mirror
URLs (`customers`, plural). The domain stays feature-first (`customer/`) and HTTP-free. The web
modules play the part `routes.rs` used to: extraction, view-model flattening, rendering.

Module routing rules that matter here:
- Every module below the root adds one segment; names are kebab-cased.
- `module_param!(pub(crate) customer_id: i64, error = bad_request("Invalid customer id."))` makes
  the module a `{customer_id}` segment and defines `CustomerId`, read with `path_param::<CustomerId>(cx)`.
- A static sibling (`new`) next to a param module (`customer_id`) works: `/customers/new` wins.
- Handlers in one module share its path, one per method: `#[page]` (GET, wrapped in layouts),
  `#[route(POST)]`, `#[route(PUT)]`, `#[route(DELETE)]` (not wrapped - bare fragments for htmx).
- Link with `href!(path::to::handler, ParamType(value))`, never hand-built strings. In a component,
  `.resolve(cx)` it into a `String` first (see the temporaries gotcha).

### Layering rules

- **Web (`app/…`)** - extraction (`Form<T>`, `path_param`), handlers, view models, components. Flatten
  domain structs into owned view models (`CustomerRow` with formatted dates) here. No SQL, no business
  rules. Get the pool with `app::pool(cx)` (registered as app context in `router()`).
- **`services.rs`** - validation and business logic; takes `*Form`, returns DTOs or `AppError`. No
  Topcoat types. May call another feature's *services* (never its repositories).
- **`repositories.rs`** - compile-time-checked `sqlx::query!`/`query_as!` only.

Cross-feature composition, delete guards and transactions are unchanged from the Axum project:
- An order validates its customer/products through their services, remaps their `NotFound` to
  `Validation` (it's a bad submission, not a missing resource) and resolves prices server-side.
- Customers/products refuse deletion while orders reference them, via
  `order::services::count_for_*`, as a friendly `AppError::Validation` instead of an FK 500. Mind
  the subject-verb agreement in counted messages ("1 order still references", "2 orders still reference").
- Multi-table writes share one `sqlx::Transaction`; an edit that submits the full child set is
  delete-then-reinsert inside it.

## Error handling: one root layer

`AppError` (`Validation`, `NotFound(&'static str)`, `Database`) implements `std::error::Error`, so
handlers return `topcoat::Result<T>` and just use `?`. Nothing in a handler builds an error response.

`src/app.rs` has one `#[layer]` (path `/`, so it wraps every handler and its layouts) that runs
`next.run(cx, body)` and, on `Err`, downcasts:
- `AppError` → its status and message
- `NotFoundError` (from `not_found!()`) → 404 "Page not found."
- `BadRequestError` (from `module_param!` parse failures) → 400 with its description
- **anything else is returned unchanged** - redirects (`SeeOther`), rewrites and unknown errors are
  errors too, and must reach Topcoat's own handling.

It then renders by request type, using `htmx::hx_request(cx)`:
- htmx → `(HxReswap(SwapOption::None), view!{ (status) ui::alert(..) })`: the element that made the
  request is left alone, and the alert swaps out-of-band into both `#alert-region` and
  `#dialog-alert-region`.
- navigation → `view!{ (status) ui::shell(title: .., ui::error_page(..)) }`, a full branded page.

A `StatusCode` in node position inside `view!` sets the response status. That is how a view-built
response carries its 4xx.

## Testing (TDG protocol)

- Unit tests in `#[cfg(test)] mod tests` beside the code, mostly service validation. Keep "shape"
  validation pure (no `&PgPool`) and split the DB-dependent "resolve" step out.
- Integration tests drive the real router: `customer_crud_topcoat::router(pool, assets)`, then
  `router.handle(request).await` (it's public, so no tower adapter is needed) and
  `topcoat::router::to_bytes` for the body. `#[sqlx::test]` gives each test a fresh, migrated
  database (needs `CREATEDB`).
- **Assets in tests**: `tests/common/mod.rs` bundles the test binary's *own* `asset!` declarations
  once (`topcoat_asset::Bundler`, a dev-dependency with the `bundler` feature) into a directory next
  to it and loads that. All local files, so no network.
- Seed related rows through the real create routes, and **anchor id extraction on unique text**
  (`row_id_near(html, "product-row-", sku)`). Create responses re-render the whole sorted list, so
  "first `product-row-`" can be the wrong row when names tie. This bit twice: once in the Axum
  project, once again in this port.
- Assert the htmx contract, not just the status: `hx-reswap: none` on errors, that fragment routes
  aren't wrapped in `<html>`, and that `hx-post`/`hx-put` URLs are what `href!` should produce.

## UI pattern: dialog-based CRUD with htmx

- One `<dialog id="crud-dialog">` in `ui::shell`, with an empty `#dialog-content`. "Add" `hx-get`s
  `/<resource>/new` and "Edit" `hx-get`s `/<resource>/{id}/edit` into it. app.js opens the dialog on
  `htmx:after:swap` when `ctx.target` is `#dialog-content` (v4 event names - see `htmx4`).
- **One form component per resource** with `#[default] existing: Option<&Row>`: `None` renders the
  create form (`#create-x-form`, `hx-post` → refresh `#rows-body`), `Some` renders the edit form
  (`#edit-x-form`, `hx-put` → swap `#x-row-<id>` outerHTML). The distinct ids still matter: app.js
  routes post-submit behaviour by form id, one named handler per resource and action.
- The list component handles its own empty state inside `<tbody id="rows-body">`, so the swap target
  always exists.
- `DELETE` returns an empty 200, and the row's `hx-swap="outerHTML"` removes it.
- `view!` compares fine inline (`selected=(Some(c.id) == data.customer_id)`), so the Askama-era
  "precompute `selected` in Rust" workaround is gone. Boolean attributes take a `bool`; optional ones
  take an `Option` (`value=(existing.map(|c| c.email.as_str()))` omits the attribute on create).
- Valueless markers need an explicit empty string: `data-line-items=""`, `required=""`.
- The nav highlight and page `<title>` come from the request path (`ui::Section::from_path`), so pages
  don't thread a `current_page` value around.

## Paged lists with server-side selection and bulk delete

Ported from `loan-onboarding-poc` (FastAPI + Redis) per the global `list-pagination-bulk-actions`
skill, with Postgres + `topcoat::session` in place of Redis. Full design and as-built notes:
`docs/PAGINATION_BULK_ACTIONS_PLAN.md`. The shape, per resource:
- `<res>_list(listing: Page<Row>, selected, #[default] oob)` renders the `<div id="<res>-list">`
  region (hidden `page`/`query_id` inputs at `#<res>-list-state`, the table, the pager). Rows and the
  toolbar `hx-include` that state, so rows never need to know their page.
- `POST /<res>/page`, `/selection`, `/bulk-delete/confirm`, `/bulk-delete`. Responses that change
  the selection send `<res>_toolbar(oob: true)`.
- Domain helpers with no Topcoat types: `pagination::fetch_page` (count cache), `selection::*`
  (keyed by the session token hash bytes), `bulk::{prepare_batch, fan_out}`.

Gotchas hit while building it (each cost a compile or test cycle):
- **`impl AsyncFn` in a helper called from a handler** fails Topcoat's `Send` bound ("implementation
  of `Send` is not general enough"): the per-call future borrows the closure for a higher-ranked
  lifetime. Take `F: Fn(i64) -> Fut, Fut: Future<..>` instead. `AsyncFnOnce` is fine.
- **htmx v4 puts `GET`/`DELETE` params in the query string** (bundle-verified), so a `DELETE` that
  `hx-include`s state reads it with `#[query_params]`, not `Form<T>`.
- **`hx-include="#div"`** sends every `[name]` input inside a non-form element (bundle-verified).
- `topcoat::session`'s cookie is always `__Host-session; Secure`; Chrome stores it on
  `http://127.0.0.1` fine. Calling `session::start` sets a cookie, so do it before building the view.
- A boolean attribute renders as `disabled=""`; tests must match that exact string, since Tailwind
  class lists contain `disabled:` variants.
- `#[serde(flatten)]` with urlencoded forms breaks numeric fields; spell the fields out.
- Bare `<tr>` outerHTML swaps (the edit-row flow) work in Chrome with htmx 4.0.0, despite the
  `htmx4` skill's §1 warning; new list swaps still target `<div>` regions.

## Dynamic repeatable form rows (order line items)

Same pattern as before, with less glue:
- Row controls have no `name`. A hidden `items_json` input carries `[{product_id, quantity}, …]`,
  kept in sync by delegated JS listeners. The service parses it with `serde_json` and turns failures
  into `AppError::Validation`.
- One `line_item_row(products, selected, quantity)` component serves three call sites: the create
  form's blank row, the edit form's loop over existing items, and an inert
  `<template data-line-item-template="">` that app.js clones for "Add item".
- The edit form renders `items_json` already in sync with its rows; the create form starts at `[]`,
  and the required product `<select>` forces an input event before submit.
- JS refuses to remove the last row; the server validates "at least one item" anyway.

## Bootstrapping checklist

1. Rust ≥ 1.98; `cargo install topcoat-cli sqlx-cli`
2. `cargo new`, edition 2024; add `topcoat` with the feature list above, plus as a build-dependency
   (`default-features = false, features = ["tailwind"]`), and `topcoat-asset` with `bundler` as a
   dev-dependency
3. `build.rs` + `static/input.css` (`@import "tailwindcss" source(none); @source "../src";`)
4. `src/app.rs` with `router()`, the layout, the error layer and `not_found!()` from the start
5. Domain modules per resource; web modules mirroring URLs
6. `sqlx migrate add/run`, `cargo sqlx prepare`, commit `.sqlx/`
7. `topcoat dev` while building; `topcoat fmt` + `cargo fmt` before committing
8. `cargo clippy --all-targets -- -D warnings && cargo test` before calling anything done
