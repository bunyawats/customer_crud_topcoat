# List search: implementation plan

Status: **planned** (2026-10-15), nothing built yet. This file is the single source of truth for
this feature's progress. Tick boxes as tasks land; add a line to the Session log at the end of
every session.

## How a session should use this file

1. Read "Decisions" and "Design" below; don't re-litigate them (they were chosen with the user).
2. Load skills: `topcoat` (global), `list-pagination-bulk-actions` (global, count-cache rules),
   `htmx4` (global, wire mechanics), `topcoat-htmx-crud-stack` (project).
3. Read the "As built" notes in [`PAGINATION_BULK_ACTIONS_PLAN.md`](PAGINATION_BULK_ACTIONS_PLAN.md):
   search rides on that feature's regions, `query_id` cache and selection store.
4. Find the first unticked task. Do it with its tests. Verify (`cargo test`, clippy). Tick it.
5. If a task changes a design decision, update this file's Design section, not just the code.
6. Log the session at the bottom: date, tasks done, anything surprising.

**Reference (read-only, never edit):** `~/Projects/Python/loan-onboarding-poc`. It has **no search
box**. Its "filter" is fixed per screen (`application/service.py` `list_by_status` per staff role,
`list_for_applicant` for the customer portal; lines ~336-434), and what we port from it is how a
filter interacts with the count cache: the cached entry stores the filter, and a `query_id` is
trusted only for the identical filter (`_lookup_cached_total`). The rules are written up in the
global `list-pagination-bulk-actions` skill, `references/count-cache-pagination.md`.

## Decisions (made with the user, 2026-10-15)

| Question | Decision |
|---|---|
| What it matches | **One free-text box per list**, case-insensitive "contains". Customers: name, email, phone. Products: name, SKU, description. Orders: order number (exact) or customer name |
| Selection when the search changes | **Keep it**, same as Prev/Next. The toolbar counts every selected row; bulk delete acts on all of them, including rows the current search hides |
| When it runs | **On Enter or a Search button** (no search-as-you-type). A clear button resets it |
| Search in the URL (`/customers?q=ada`) | **Open: confirm in task 0.1.** Default if unanswered: yes (see Phase 4) |

Decisions taken by the planner, not the user (change them freely if the user disagrees):
- **Create clears the search.** After "Add customer" the list re-renders unfiltered on the page
  holding the new row, and the search box empties. Otherwise a new row that doesn't match would
  silently vanish.
- **Edit doesn't re-filter.** `PUT` still swaps just the row; if the edit makes it stop matching,
  it stays visible until the next search or page change. Cheap and unsurprising.
- **No new index.** `ILIKE '%term%'` can't use the existing btree indexes; at POC size a sequential
  scan is fine. `pg_trgm` GIN indexes are a follow-up, not part of this plan.

## Design

### Search term (domain, `src/pagination.rs`)
- New `Search` value type: built from raw input by trimming, collapsing inner whitespace, and
  capping at **100 chars** (longer → `AppError::Validation("Search must be at most 100
  characters.")`). Empty after trimming = no filter.
- `Search::like_pattern() -> Option<String>`: `%term%` with `\`, `%` and `_` escaped
  (Postgres `LIKE`'s default escape is `\`), so a user typing `50%` matches literally.
- `PageRequest` gains `search: Search`. Keeping it inside the request means every existing
  re-render site (Prev/Next, selection, single delete, bulk delete) carries the search with no
  signature changes; only the form structs that build a `PageRequest` gain a `q` field.
- `fetch_page` builds the cache key from base + term (`"customers"` → `"customers?q=ada"`), so a
  `query_id` minted under another search is a miss and recomputes (the reference's rule: a freshly
  supplied filter always bypasses the cache). `Page` carries the `Search` back so views can render
  it.

### Queries (repositories)
All filtered by one optional pattern parameter, so filtered and unfiltered share one compile-checked
query: `WHERE $1::text IS NULL OR …`.

| List | Predicate (`$1` = escaped pattern) |
|---|---|
| Customers | `first_name \|\| ' ' \|\| last_name ILIKE $1 OR email ILIKE $1 OR phone ILIKE $1` |
| Products | `name ILIKE $1 OR sku ILIKE $1 OR description ILIKE $1` |
| Orders | `o.id = $2` (when the term, minus a leading `#`, is all digits) `OR c.first_name \|\| ' ' \|\| c.last_name ILIKE $1` |

`count` and `list_page` take the pattern; `position_of` (used only by create) stays unfiltered,
because create clears the search. `ORDER BY` and the `id` tiebreakers are unchanged.

### Web (`src/app/`, `src/ui.rs`)
- **Where the active search lives:** a third hidden input, `q`, in `#<res>-list-state`, next to
  `page` and `query_id`. Everything that already `hx-include`s the state (Prev/Next, row delete,
  row checkbox, select-all, bulk delete) then sends the *executed* search automatically, never the
  box's unsubmitted text.
- **The search box:** new `ui::search_form` component in its own region `#<res>-search`, between
  the page header and the toolbar. It is **outside** `#<res>-list` and the toolbar, so neither
  re-render touches what the user typed. A `<form>` with `hx-post="/<res>/page"`,
  `hx-target="#<res>-list"`, `hx-swap="outerHTML"`, fields `q` and `page=1`, no `query_id` (fresh
  count). `type="search"` input with a label, a Search button, and a clear button that posts
  `q=""`.
- **Forms:** `PageForm`, `SelectionForm` and `ListQuery` (the `DELETE` query-string fields) gain
  `#[serde(default)] q: String`; `PageForm::after_write` keeps `q`. Bulk delete keeps it too.
- **Create:** responds with the unfiltered list (page holding the new row) plus an OOB
  `#<res>-search` with an empty box.
- **Empty states:** with a search and no matches: "No customers match "ada"." plus a hint to clear
  the search. Without a search, the existing "nothing here yet" row.
- **Pager text:** unchanged ("Showing 1–3 of 3"); the count is the filtered count.
- **Bulk-delete confirm:** lists every selected row (already re-read server-side). Add one line
  when some are outside the current search: "2 of these aren't in the current search." The confirm
  route needs `q` from the state to compute that.

### Rules carried over (don't break)
- A list `#[page]` visit clears the selection; a search never does.
- Bulk routes re-read ids from `selected_items`, never from the client.
- `query_id` is an optimisation only: an unknown or mismatched one recomputes, never errors.
- htmx v4 sends `DELETE` params in the query string: `q` for row delete is read with
  `#[query_params]`, like `page`.

## Tasks

### Phase 0: confirm
- [ ] **0.1** Ask the user the open URL question (Decisions table). Record the answer there and
  tick or strike Phase 4.

### Phase 1: domain
- [ ] **1.1** `pagination::Search` (normalise, cap, `like_pattern` escaping), `PageRequest.search`,
  search-aware cache key in `fetch_page`, `Page.search`. Unit tests: trim/collapse, empty = none,
  101 chars rejected, `%`/`_`/`\` escaped, cache miss when the same `query_id` comes with another
  term, hit with the same term.
- [ ] **1.2** Repositories + services for all three lists: pattern parameter on `count` and
  `list_page`; orders' numeric-id branch (`Search::order_id()` or similar, unit-tested: `"12"`,
  `"#12"`, `"12a"`, `" 12 "`). `cargo sqlx prepare`; commit `.sqlx/`.

### Phase 2: web
- [ ] **2.1** `ui::search_form` + `#<res>-search` region on all three index pages; `q` in
  `ui::list_state`; `q` on `PageForm`, `SelectionForm`, `ListQuery`.
- [ ] **2.2** Create clears the search (OOB empty search box, unfiltered list on the new row's
  page). Search-aware empty-state row.
- [ ] **2.3** Bulk-delete confirm: the "N of these aren't in the current search" line.

### Phase 3: tests (`tests/search_routes.rs`, new)
- [ ] **3.1** Matching: each field per list, case-insensitive, `%`/`_` literal, order `#12` and
  `12`, no-match empty state, 101-char term → 422 alert.
- [ ] **3.2** Paging under a search: filtered total in the pager, Prev/Next keep `q` (via the
  state), a `query_id` from another search isn't reused (count differs), page clamps within the
  filtered total.
- [ ] **3.3** Writes under a search: row delete re-renders the same search and page; bulk delete
  deletes selected rows hidden by the search; create clears the search and shows the new row.
- [ ] **3.4** Selection: ticks survive a search change and clearing it; the select-all box reflects
  only the filtered page's ids.

### Phase 4: URL (only if task 0.1 says yes)
- [ ] **4.1** Index `#[page]`s read `q` with `#[query_params]` and render the filtered list and a
  pre-filled box; the search form adds `hx-push-url` with `?q=`. Note the consequence in the UI
  docs: Back/reload is a page visit, so it clears the selection (existing rule). Tests: `GET
  /customers?q=ada` filters; the page route never pushes a URL for Prev/Next.

### Phase 5: verify and document
- [ ] **5.1** Browser walkthrough (`PORT=3010 topcoat dev`, seeded data): search, Prev/Next within
  results, tick rows across two searches, bulk delete with hidden selected rows, clear, create
  while searching. Use element refs, not coordinates (hx-confirm dialogs block the extension).
- [ ] **5.2** Docs: new `docs/interaction-diagrams/14-post-search.md` (or extend `03-post-page.md`
  if it's the same route), update `README` index, CLAUDE.md "Lists" section, project skill (both
  copies), architecture page only if a box changes. ER diagram: no change (no migration).
- [ ] **5.3** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`,
  `topcoat fmt`, `SQLX_OFFLINE=true cargo check`.

## Follow-ups (not planned work)
- `pg_trgm` GIN indexes for the `ILIKE` columns if the lists grow.
- Search-as-you-type (`hx-trigger="input changed delay:300ms, search"`) if the user wants it later;
  the box would then need `hx-sync` to drop stale responses.

## Session log
- **2026-10-15:** plan written. Reference checked: no search UI, only fixed per-role filters; its
  filter-aware count cache is the part to port. Decisions taken with the user (text search, keep
  selection, Enter/button); the URL question was left unanswered.
