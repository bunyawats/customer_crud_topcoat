# Pagination + multi-select + bulk delete: implementation plan

Status: **planned, not started** (2026-10-15). This file is the single source of truth for this
feature's progress. Tick boxes as tasks land; add a line to the Session log at the end of every
session.

## How a session should use this file

1. Read "Decisions" and "Design" below; don't re-litigate them (they were chosen with the user).
2. Load skills: `topcoat` (global), `list-pagination-bulk-actions` (global, architecture),
   `htmx4` (global, wire mechanics), `topcoat-htmx-crud-stack` (project).
3. Find the first unticked task. Do it with its tests. Verify (`cargo test`, clippy). Tick it.
4. If a task changes a design decision, update this file's Design section, not just the code.
5. Log the session at the bottom: date, tasks done, anything surprising.

**Reference implementation (read-only, never edit):** `~/Projects/Python/loan-onboarding-poc`, a
FastAPI + HTMX + Redis build of the same feature.
- `loan_onboarding/application/service.py` (lines ~336-434): `query_id` count cache, `list_by_status`
- `loan_onboarding/bff_backoffice/routes.py`: `_resolve_page`, `_toolbar_oob`, `_staff_bulk_select`,
  `_bulk_decision_form` (confirm), `_bulk_decision_execute`, `_bulk_result_response`
- `loan_onboarding/bff_backoffice/selection_store.py`: selection keyed by session
- `loan_onboarding/bff_backoffice/templates/`: `_bulk_toolbar.html`, `_staff_list.html` (pager),
  `_staff_row.html` (checkbox), `_bulk_confirm_dialog.html`, `_bulk_result_dialog.html`

## Decisions (made with the user, 2026-10-15)

| Question | Decision |
|---|---|
| Which lists | **All three**: customers, products, orders, built on shared components |
| Bulk actions | **Bulk delete** on each list, fanned out over the existing single-item delete |
| Selection storage | **Postgres table**, keyed by an anonymous **Topcoat session** token hash. No Redis |
| Page size | 10 (the reference's value) |
| Count-cache TTL | 30 s, in-process, pure optimisation (a miss always recomputes) |
| Atomic or best-effort | Best-effort per item; result dialog lists failures (e.g. customer with orders) |
| Batch cap | 100 ids; over the cap → `AppError::Validation`, nothing deleted |
| Who can select vs act | No auth in this app; the existing per-item guards are the only checks |

## Facts verified while planning (don't re-check unless versions change)

- `topcoat::session` only manages tokens: `session::start(cx)` sets the cookie and returns a
  `Session { token_hash, expires_at }`; `session::token_hash(cx)` reads it (`Option<TokenHash>`,
  derefs to `[u8; 32]`). Storage is ours. Needs features `session` + `cookie` and
  `.cookies().sessions(SessionConfig::default())` on the router builder.
- The session cookie is **always** `__Host-session`, `Secure`, `HttpOnly`, `SameSite=Lax`
  (`topcoat-session-0.10.0/src/token/store.rs`). Chrome accepts Secure cookies on
  `http://127.0.0.1`/`localhost`, but **confirm in the browser in task 0.2**. Fallback: implement
  the public `TokenStore` trait with a non-Secure cookie for dev.
- Cookie writes must happen before the response body streams: call `session::start` in the handler
  before building the view.
- htmx bundle at `unpkg.com/htmx.org@next` is byte-identical to `4.0.0` (checked 2026-10-15).
- `hx-vals='js:…'`: `js:` is stripped and the rest evaluated. Arrays become one comma-joined field
  (htmx4 skill), so send ids as `"1,2,3"` explicitly.
- `hx-include="#x"` on a non-form element includes every `[name]` descendant (bundle `#Ze`). Used to
  send the current `page`/`query_id` from hidden inputs, so rows don't need page info baked in.
- htmx v4 strips `<tr>`/`<td>` from bare row responses (htmx4 skill §1). New swaps target `<div>`
  regions only. The existing edit-row swap (`PUT` → bare `<tr>`, outerHTML) may already be affected:
  check in task 0.1.

## Design

### Domain (no Topcoat types)

- `src/pagination.rs` (new, shared): `Page<T> { items, total, page, page_size, query_id }`,
  `PageRequest { page, query_id }`, and the count cache (`query_id` → `(filter_key, total,
  expires_at)` in a `Mutex<HashMap>`; mint `q_<12 hex>`; a miss, expiry or different filter_key
  recomputes; clamp page ≥ 1 and page beyond the last page to the last page).
- Each resource's `repositories.rs`: `count(pool)`, `list_page(pool, limit, offset)`, and
  `position_of(pool, id)` (0-based rank in the list order, so create can show the page holding the
  new record). **List orders get an `id` tiebreaker** so `OFFSET` paging is stable:
  customers `last_name, first_name, id`; products `name, id`; orders `created_at DESC, id DESC`
  (already done).
- Each resource's `services.rs`: `list_page(pool, PageRequest) -> Page<_>`,
  `page_of(pool, id) -> i64`, and `bulk_delete(pool, ids) -> Vec<BulkResult>` that de-dupes
  (order-preserving), rejects empty / > 100, then calls the existing `delete` per id, mapping
  `AppError::Validation`/`NotFound` to a failed item and propagating `Database`.
- `src/selection/` (new domain module): `repositories.rs` + `services.rs` over table
  `selected_items(session_hash BYTEA, resource TEXT CHECK IN ('customers','products','orders'),
  item_id BIGINT, selected_at TIMESTAMPTZ DEFAULT now(), PRIMARY KEY (session_hash, resource,
  item_id))`. Functions: `get`, `update(ids, checked)`, `clear`, `remove(id)`. Opportunistically
  delete rows older than 1 day on write. `Resource` enum with `as_str()`.
- Unique/constraint names: none new that need `unique_violation_message` arms.

### Web (`src/app/`)

Shared, in `src/ui.rs` or a new `src/app/list.rs`:
- `session_hash(cx) -> Option<TokenHash>` (read only) and `ensure_session(cx) -> TokenHash`
  (starts one if missing; only in POST routes, before rendering).
- Components: `bulk_toolbar(resource, selected_count, oob)` (`<div id="<res>-toolbar">`, count +
  "Delete selected", disabled at 0), `pager(page)` (tfoot: "Showing a–b of N", Prev/Next),
  `list_state(page)` (`<div id="<res>-list-state">` with hidden `page` and `query_id` inputs),
  `select_all_checkbox`, `row_checkbox(id, checked)`, `bulk_confirm_dialog`,
  `bulk_result_dialog`. Reuse the existing `<dialog id="crud-dialog">` + `#dialog-content`.

Regions per list page (no polling here, so the header may be rebuilt):
- `#<res>-toolbar` (outside the list, sent OOB by every selection/list-changing response)
- `#<res>-list` (a `<div>` holding the table card: thead with select-all, tbody, tfoot pager, and
  the hidden list-state inputs). Replaced with `outerHTML`.

Routes per resource (static modules beat the `{id}` param module):

| Route | Does | Response |
|---|---|---|
| `GET /customers` (`#[page]`) | page 1; **clears this session's selection** for the resource | full page |
| `POST /customers/page` | Prev/Next: `page`, `query_id` form fields | `#customers-list` + toolbar OOB |
| `POST /customers/selection` | `ids="1,2"`, `checked`, + included `page`/`query_id`; starts session if needed | `#customers-list` + toolbar OOB (row checkbox uses `hx-swap="none"`, select-all targets the list) |
| `POST /customers/bulk-delete/confirm` | reads selection server-side, fetches fresh display fields, drops stale ids | confirm dialog into `#dialog-content` |
| `POST /customers/bulk-delete` | re-reads selection (never client ids), fans out delete, **clears selection** | result dialog + `#customers-list` OOB + toolbar OOB |
| `POST /customers` (existing create) | after create, render the page containing the new row (`page_of`) | change target from `#rows-body` to `#customers-list` |
| `DELETE /customers/{id}` (existing) | also remove id from selection; re-render current page via `hx-include` state | target `#customers-list` instead of the row |
| `PUT /customers/{id}` (existing edit) | row keeps its checkbox state (look up selection) | unchanged shape, fixed per task 0.1 |

Same table for `/products` and `/orders`. Delete guards already produce
`AppError::Validation` ("… still referenced by N orders"), which bulk delete reports per item.

### Tests

- Harness (`tests/common/mod.rs`): add a cookie jar to `App` (store `Set-Cookie` values, send them
  as `Cookie`), plus helpers `select(resource, ids, checked)` and `seed_customers(n)`.
- Per resource (`tests/<res>_routes.rs` or a new `tests/bulk_routes.rs`): page 2 shows rows 11–20;
  Prev disabled on page 1 / Next disabled on the last page; page past the end clamps; selection
  survives Prev/Next (checkbox `checked` on return); select-all on a page; toolbar count; confirm
  lists fresh names; bulk delete deletes, clears selection, reports a guarded customer as failed;
  over-cap rejected; selection isolated between two sessions (two `App` cookie jars); fresh `GET`
  clears selection; create lands on the page holding the new record.
- Unit tests: count cache (hit, expiry, filter mismatch, unknown id), id parsing (`""`, `"1,,2"`,
  non-numeric → validation), de-dupe + cap, page clamping, `page_of` arithmetic.

## Tasks

### Phase 0: de-risk (do first, small)
- [ ] **0.1** In the browser (`PORT=3010 topcoat dev`), edit a customer and check whether the row
  re-renders correctly or its cells collapse (htmx v4 bare-`<tr>` stripping). If broken, fix it the
  htmx4-skill way (wrap row responses in `<table><tbody>…</tbody></table>`, add `hx-select="tr"` on
  the edit form) with a test asserting the wrapper. Record the result in the Session log.
- [ ] **0.2** Spike: enable `session` + `cookie` features and `.cookies().sessions(..)`, add a
  throwaway route that calls `session::start`, and confirm in Chrome that `__Host-session` is
  stored on `http://127.0.0.1:3010` and sent back. If not, implement a dev `TokenStore`. Remove the
  throwaway route.

### Phase 1: pagination (no selection yet)
- [ ] **1.1** `src/pagination.rs`: `Page`, `PageRequest`, count cache, clamping + unit tests.
- [ ] **1.2** Repos/services: `count`, `list_page`, `position_of`/`page_of`, id tiebreakers for all
  three resources. `cargo sqlx prepare`; commit `.sqlx/`.
- [ ] **1.3** UI: `#<res>-list` region, `pager`, `list_state`; `POST /<res>/page` for all three.
  Rewire create (page holding the new row) and single delete (re-render current page).
- [ ] **1.4** Integration tests for paging, clamping, create landing page, delete re-render.
  Browser check of Prev/Next on 25+ seeded customers (extend `seeds/dev.sql` or seed via a test
  script, but keep `seeds/dev.sql` idempotent).

### Phase 2: selection
- [ ] **2.1** Migration for `selected_items`; `src/selection/` module + repo tests (`#[sqlx::test]`).
- [ ] **2.2** Session helpers; harness cookie jar.
- [ ] **2.3** Row checkboxes, select-all, `#<res>-toolbar`; `POST /<res>/selection`; page GET clears
  selection; edit row keeps state.
- [ ] **2.4** Integration tests: survive paging, select-all, count, session isolation, GET clears.

### Phase 3: bulk delete
- [ ] **3.1** `bulk_delete` service per resource (de-dupe, cap, fan-out, per-item results) + unit
  tests.
- [ ] **3.2** Confirm dialog route + component; execute route + result dialog; clear selection;
  OOB list + toolbar refresh.
- [ ] **3.3** Integration tests incl. guarded customer/product reported as failed, stale ids dropped.

### Phase 4: finish
- [ ] **4.1** Browser walkthrough on all three lists (select across pages, bulk delete with a
  partial failure), screenshots.
- [ ] **4.2** Docs: CLAUDE.md (new routes, `selection/` module, `pagination.rs`, session features),
  project skill `topcoat-htmx-crud-stack` if a new gotcha came up, README feature list.
- [ ] **4.3** `cargo fmt`, `topcoat fmt`, clippy, full `cargo test`; commit.

## Session log

- 2026-10-15: plan written. Decisions taken with the user; reference read; Topcoat session/cookie
  and htmx 4.0.0 `hx-vals`/`hx-include` behaviour verified (see "Facts verified"). No code yet.
