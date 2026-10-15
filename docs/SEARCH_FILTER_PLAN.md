# List search: implementation plan

Status: **done** (2026-10-15). Follow-ups are listed near the end. This file is the single source of truth for
this feature's progress. Tick boxes as tasks land; add a line to the Session log at the end of
every session.

## How a session should use this file

1. Read "Decisions" and "Design" below; don't re-litigate them (they were chosen with the user).
2. Load skills: `topcoat` (global), `list-pagination-bulk-actions` (global, count-cache and
   session-store rules), `htmx4` (global, wire mechanics), `topcoat-htmx-crud-stack` (project).
3. Read the "As built" notes in [`PAGINATION_BULK_ACTIONS_PLAN.md`](PAGINATION_BULK_ACTIONS_PLAN.md):
   search rides on that feature's regions, `query_id` cache and session-keyed selection store.
4. Find the first unticked task. Do it with its tests. Verify (`cargo test`, clippy). Tick it.
5. If a task changes a design decision, update this file's Design section, not just the code.
6. Log the session at the bottom: date, tasks done, anything surprising.

**Reference (read-only, never edit):** `~/Projects/Python/loan-onboarding-poc`.
- Lists are driven by `POST` (`bff_backoffice/routes.py:360` `POST /ui/<role>/list` with
  `page`/`query_id`); we do the same.
- Its filter is fixed per screen (`application/service.py` ~336-434: `list_by_status` per staff
  role, `list_for_applicant`), so it never stores criteria. Its count cache keeps the filter next
  to the total and trusts a `query_id` only for the identical filter (`_lookup_cached_total`).
- Its server-side stores (`bff_backoffice/session_store.py`, `selection_store.py`, Redis) are keyed
  by session. Our filter store follows that shape, as the global skill suggests ("the server-side
  session store can hold a per-session `(filter, total)`", `references/count-cache-pagination.md`).

## Decisions (made with the user, 2026-10-15)

| Question | Decision |
|---|---|
| What it matches | **One free-text box per list**, case-insensitive "contains". Customers: name, email, phone. Products: name, SKU, description. Orders: order number (exact) or customer name |
| Where the criteria live | **A server-side filter object per session and list**, like the selection. The browser posts the criteria once, with the search; every later request (Prev/Next, ticks, deletes) reads them from the server |
| Request method | **`POST`**, like the reference's list route |
| Selection when the search changes | **Keep it**, same as Prev/Next. The toolbar counts every selected row; bulk delete acts on all of them, including rows the current search hides |
| When it runs | **On Enter or a Search button** (no search-as-you-type). A clear button resets it |
| Search in the URL | **No**: the server keeps it (follows from "criteria live server-side") |

Decisions taken by the planner, not the user (change them freely if the user disagrees):
- **A page visit keeps the filter, clears only the ticks.** Reload or coming back to `/customers`
  shows the last search, pre-filled, until the clear button. That's the point of storing it.
  Ticks still clear on a visit (existing rule).
- **Create clears the filter.** After "Add customer" the list re-renders unfiltered on the page
  holding the new row and the box empties. Otherwise a new row that doesn't match would silently
  vanish.
- **Edit doesn't re-filter.** `PUT` still swaps just the row; if it stops matching, it stays until
  the next search or page change.
- **Two tabs share one filter per list**, as they already share the selection (same session).
- **No new search index.** `ILIKE '%term%'` can't use btree indexes; fine at POC size. `pg_trgm`
  is a follow-up.

## Design

### The filter object (domain, new `src/filter/`, no HTTP types)
- `ListFilter { search: Search }`, `Serialize`/`Deserialize`, `Default` = no filter. One struct
  for all three lists today; new criteria (e.g. an order-status dropdown) become new fields with
  `#[serde(default)]`, so stored rows from older code still load.
- `Search`: built from raw input by trimming, collapsing inner whitespace, capping at **100 chars**
  (longer → `AppError::Validation("Search must be at most 100 characters.")`). Empty = no filter.
  `like_pattern() -> Option<String>`: `%term%` with `\`, `%`, `_` escaped (Postgres `LIKE`'s
  default escape is `\`). `order_id() -> Option<i64>` for `"12"`/`"#12"`.
- `ListFilter::cache_key(base)`: a canonical string (`"customers?q=ada"`) used as `fetch_page`'s
  `filter_key`, so a `query_id` minted under other criteria is a miss and recomputes.
- **Store:** migration `create_list_filters`:
  ```sql
  CREATE TABLE list_filters (
      session_hash BYTEA NOT NULL,
      resource     TEXT NOT NULL CHECK (resource IN ('customers', 'products', 'orders')),
      criteria     JSONB NOT NULL,
      updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
      PRIMARY KEY (session_hash, resource)
  );
  CREATE INDEX idx_list_filters_updated_at ON list_filters (updated_at);
  ```
  **As built:** no sqlx `json` feature; the repository passes and reads the JSON as text
  (`$3::text::jsonb`, `criteria::text`) and the service owns serde. Repositories `get`/`upsert`/`clear`/`purge_older_than`; services
  `get` (a missing row, no session, or criteria that fail to deserialize → `ListFilter::default()`:
  **fail open to "no filter"**, never an error page), `set` (an empty filter deletes the row),
  `clear`. Rows older than one day are purged on the next write, like `selected_items`.
- `selection::models::Resource` is reused as the resource key (move it somewhere shared if that
  reads better).

### Queries (repositories)
Filtered and unfiltered share one compile-checked query: `WHERE $1::text IS NULL OR …`.

| List | Predicate (`$1` = escaped pattern, `$2` = order id or NULL) |
|---|---|
| Customers | `first_name \|\| ' ' \|\| last_name ILIKE $1 OR email ILIKE $1 OR phone ILIKE $1` |
| Products | `name ILIKE $1 OR sku ILIKE $1 OR description ILIKE $1` |
| Orders | `o.id = $2 OR c.first_name \|\| ' ' \|\| c.last_name ILIKE $1` |

Services' `list_page(pool, request, &filter)` pass the pattern to `count` and `list_page` and the
cache key to `fetch_page`. `position_of` (create only) stays unfiltered. `ORDER BY` unchanged.

### Web (`src/app/`, `src/ui.rs`)
- **New route `POST /<res>/search`** (`app/<res>/search.rs`), form field `q`. It validates `q`,
  starts the session if needed (`ensure_session_key`, before building the view), saves the filter
  (empty `q` clears it), and renders `#<res>-list` on page 1 with a fresh count. The browser never
  sends criteria anywhere else.
- **Every other list re-render reads the stored filter**: `GET /<res>` (index), `POST /<res>/page`,
  `/selection`, row `DELETE`, `/bulk-delete` and its confirm. A shared helper in `app.rs`
  (`list_filter(cx, resource)`, read-only like `selected_ids`) returns the default with no
  session. `#<res>-list-state` stays `page` + `query_id`; no criteria in the page.
- **Search box:** `ui::search_form` in its own region `#<res>-search` between the page header and
  the toolbar, outside `#<res>-list` and the toolbar so their re-renders never touch what the user
  typed. `hx-post="/<res>/search"`, `hx-target="#<res>-list"`, `hx-swap="outerHTML"`. A labelled
  `type="search"` input pre-filled from the stored filter, a Search button and a clear button
  (posts `q=""`).
- **Index page:** clears the ticks (as today), keeps and applies the stored filter, pre-fills the box.
- **Create:** clears the stored filter; responds with the unfiltered list on the new row's page
  plus an OOB `#<res>-search` with an empty box.
- **Empty state:** with a filter and no matches, "No customers match "ada"." plus a hint to clear.
  Without one, the existing "nothing here yet" row.
- **Bulk-delete confirm:** lists every selected row (already re-read server-side), plus one line
  when some fall outside the stored filter: "2 of these aren't in the current search."
- Visiting a list still starts no session (existing test `has_session`); only `/selection` and
  `/search` do.

### Rules carried over (don't break)
- A list `#[page]` visit clears the ticks; a search never does.
- Bulk routes re-read ids from `selected_items`, never from the client; list routes read criteria
  from `list_filters`, never from the client (only `/search` accepts them).
- `query_id` is an optimisation only: unknown or mismatched recomputes, never errors.
- Session-store failure modes: no session or unreadable criteria = no filter; it must never crash
  the render or hide rows.

## As built (notes for the next change)

- `Search` (de)serializes through `try_from = "String"`, so stored terms are re-validated on load;
  an overlong or non-string stored term fails open to "no filter" (logged at `warn`).
- `ListFilter::cache_key` appends the filter's JSON to the list name, so new criteria fields join
  the cache key automatically.
- The search box is `ui::search_form` inside `#<res>-search`; "Clear" sits **outside** the
  `<form>` so it posts only its own `hx-vals` `q=""` (inside the form, htmx would also send the
  typed text). Only a clear (and create) re-renders the box, out-of-band; a normal search leaves
  the box and its focus alone.
- The bulk confirm's "N of these aren't in the current search" uses each service's
  `count_outside`, one more filtered query per resource (`count_matching_ids`), only run when a
  filter is set.
- Test gotcha: a row checkbox's `hx-vals` contains `event.target.checked`, so assert on
  ` checked=""`, never a bare `checked`.

## Tasks

### Phase 1: domain
- [x] **1.1** `src/filter/`: `ListFilter`, `Search` (normalise, cap, `like_pattern`, `order_id`),
  `cache_key`. Unit tests: trim/collapse, empty = none, 101 chars rejected, `%`/`_`/`\` escaped,
  `"12"`/`"#12"`/`"12a"`/`" 12 "`, cache key differs per term and is stable for equal filters,
  deserializing `{}` and unknown fields gives a usable filter.
- [x] **1.2** Migration `list_filters`, sqlx `json` feature, filter repositories + services
  (`get`/`set`/`clear`, fail-open on bad JSON, purge). Update `docs/er-diagram.md` in the same
  change. Repository tests with `#[sqlx::test]`.
- [x] **1.3** Lists: pattern parameter on `count`/`list_page` for all three, services take
  `&ListFilter`, cache key passed to `fetch_page`. `cargo sqlx prepare`; commit `.sqlx/`.

### Phase 2: web
- [x] **2.1** `app::list_filter` helper; every list re-render site reads it (index, page,
  selection, row delete, bulk delete + confirm).
- [x] **2.2** `POST /<res>/search` for all three, `ui::search_form` + `#<res>-search` region,
  pre-filled box on the index page.
- [x] **2.3** Create clears the filter (OOB empty box, unfiltered list on the new row's page);
  filter-aware empty state; bulk-confirm "not in the current search" line.

### Phase 3: tests (`tests/search_routes.rs`, new; reuse the cookie-jar `App`)
- [x] **3.1** Matching: each field per list, case-insensitive, `%`/`_` literal, order `#12` and
  `12`, no-match empty state, 101-char term → 422 alert, and the stored filter is unchanged after
  a rejected term.
- [x] **3.2** Server-held criteria: after a search, Prev/Next with only `page`/`query_id` stays
  filtered; a `q` posted to `/page` is ignored; a reload (`GET /<res>`) is still filtered and
  pre-filled but has no ticks; clear restores the full list; two `App`s (browsers) on one pool
  don't see each other's filter; the three lists' filters are independent.
- [x] **3.3** Count cache: a `query_id` from before a search isn't reused after it (filtered total
  shows); page clamps within the filtered total.
- [x] **3.4** Writes: row delete re-renders the same filter and page; bulk delete deletes selected
  rows hidden by the filter; create clears the filter and shows the new row.
- [x] **3.5** Selection: ticks survive a search and its clearing; select-all covers only the
  filtered page; viewing a list still starts no session.

### Phase 4: verify and document
- [x] **4.1** Browser walkthrough (`PORT=3010 topcoat dev`, seeded data): search, Prev/Next in
  results, reload keeps the search, tick rows across two searches, bulk delete with hidden
  selected rows, clear, create while searching. Use element refs, not coordinates (hx-confirm
  dialogs block the extension). Note `topcoat dev`'s live reload re-loads open list tabs after a
  rebuild (clears ticks, keeps the filter).
- [x] **4.2** Docs: `docs/interaction-diagrams/14-post-search.md` + README index, update the
  diagrams of routes that now read the filter (02, 03, 08, 10-12), CLAUDE.md "Lists" section,
  project skill (both copies), `docs/architecture.md` (new `filter` domain box, `list_filters`
  in the database label).
- [x] **4.3** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`,
  `topcoat fmt`, `SQLX_OFFLINE=true cargo check`.

## Follow-ups (not planned work)
- `pg_trgm` GIN indexes for the `ILIKE` columns if the lists grow.
- More criteria on the filter object (e.g. order status dropdown, date range).
- Search-as-you-type (`hx-trigger="input changed delay:300ms, search"` + `hx-sync`) if wanted.

## Session log
- **2026-10-15:** plan written. Reference checked: lists are `POST`-driven; no search UI, only
  fixed per-role filters; its filter-aware count cache is the part to port. User decisions: text
  search, keep selection, Enter/button, then (revision) criteria kept server-side in a per-session
  filter object, `POST`. The URL question became moot.
- **2026-10-15 (build session):** tasks 1.1-3.5, 4.2, 4.3 done. 133 tests (20 new in
  `search_routes.rs`, 11 new unit/db tests in `src/filter/`), clippy, fmt, offline check green; all
  18 Mermaid diagrams render (mermaid 11.4.1). Surprises: a bare `contains("checked")` assertion
  passes vacuously (the checkbox's `hx-vals` says `event.target.checked`); the browser walkthrough
  couldn't run because the app tab was in the background, where Chrome throttles timers and the
  extension's clicks/screenshots time out. 4.1 needs the tab in front.
- **2026-10-15 (walkthrough):** 4.1 done in Chrome on the customers list (tab in front): search
  "sample" → 1–10 of 22, Next → 11–20 of 22, reload kept the box and filter; creating "Tmp Alpha"
  cleared the search (box empty, 29 rows); ticked Alpha under "tmp alpha" and Beta under
  "tmp beta" → toolbar "2 selected"; confirm listed both with "1 of these isn't in the current
  search."; deleted both (result dialog, list showed "No customers match “tmp beta”."); Clear →
  all 28. Database afterwards: 0 throwaway rows, 0 `list_filters`, 0 `selected_items`.
