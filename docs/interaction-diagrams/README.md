# Interaction diagrams

Every HTTP endpoint the app serves, one file per endpoint with a sequence diagram of what it does. Source of truth:
the `#[page]` / `#[route]` handlers under [`src/app/`](../../src/app/) (module path = URL). See
[`architecture.md`](../architecture.md) for the big picture and [`er-diagram.md`](../er-diagram.md) for
the tables.

Conventions used below:
- `<res>` is `customers`, `products` or `orders`; `{id}` is that resource's numeric id. The 11
  per-resource endpoints behave the same for all three; differences are listed in
  [Per-resource differences](#per-resource-differences).
- **Page** = full HTML document (wrapped in the root layout). **Fragment** = bare HTML for htmx to
  swap. **OOB** = an extra out-of-band fragment in the same response (`hx-swap-oob`).
- **Layers, in every diagram:** handler (`src/app/…`) → `<res>::services` (rules, validation) →
  `<res>::repositories` (SQLx, the only code that runs SQL) → PostgreSQL. Multi-select state goes
  the same way through `selection::services` → `selection::repositories`. Calls into another
  resource always go through its *services*, never its repositories (e.g. a customer delete asks
  `order::services::count_for_customer`).
- Every request passes through the `handle_errors` layer; the diagrams show it only where it does
  something. Its error behaviour is in [13-errors.md](13-errors.md).
- "List state" = the hidden `page` / `query_id` inputs in `#<res>-list-state`, sent with
  `hx-include` so the server can re-render the page the user is on.

## Catalogue

| # | Method | Path | Handler | Called by | Response | htmx swap |
|---|---|---|---|---|---|---|
| [1](01-get-root.md) | GET | `/` | `app::home` | navigation | 303 → `/customers` | - |
| [2](02-get-list-page.md) | GET | `/<res>` | `<res>::index` (`#[page]`) | navigation, sidebar | Page: header, toolbar, list (page 1) | - |
| [3](03-post-page.md) | POST | `/<res>/page` | `<res>::page::show_page` | Prev / Next | Fragment: list region | `#<res>-list` outerHTML |
| [4](04-get-new-form.md) | GET | `/<res>/new` | `<res>::new::new_form` | "Add …" button | Fragment: create form | `#dialog-content` innerHTML |
| [5](05-post-create.md) | POST | `/<res>` | `<res>::create` | create form submit | Fragment: list region on the new row's page | `#<res>-list` outerHTML |
| [6](06-get-edit-form.md) | GET | `/<res>/{id}/edit` | `<res>::<x>_id::edit::edit_form` | row Edit icon | Fragment: edit form | `#dialog-content` innerHTML |
| [7](07-put-update.md) | PUT | `/<res>/{id}` | `<res>::<x>_id::update` | edit form submit | Fragment: one row | `#<x>-row-{id}` outerHTML |
| [8](08-delete-row.md) | DELETE | `/<res>/{id}?page=N` | `<res>::<x>_id::delete` | row Delete icon (after `hx-confirm`) | Fragment: list region + toolbar OOB | `#<res>-list` outerHTML |
| [9](09-get-row.md) | GET | `/<res>/{id}` | `<res>::<x>_id::show_row` | **nothing in the UI** (tests only) | Fragment: one row | - |
| [10](10-post-selection.md) | POST | `/<res>/selection` | `<res>::selection::update_selection` | row checkbox, select-all | Fragment: list region + toolbar OOB | row: `none`; select-all: `#<res>-list` outerHTML |
| [11](11-post-bulk-delete-confirm.md) | POST | `/<res>/bulk-delete/confirm` | `<res>::bulk_delete::confirm::confirm_bulk_delete` | toolbar "Delete selected" | Fragment: confirm dialog | `#dialog-content` innerHTML |
| [12](12-post-bulk-delete.md) | POST | `/<res>/bulk-delete` | `<res>::bulk_delete::execute_bulk_delete` | confirm dialog submit | Fragment: result dialog + list OOB + toolbar OOB | `#dialog-content` innerHTML |
| [-](13-errors.md) | any | unrouted path | `not_found!()` catch-all | anything | 404 error page / alert | - |
| - | GET | `/assets/…` | asset bundle | `<link>`, `<script>` | CSS / JS, cache-forever | - |

<details>
<summary>All 34 concrete endpoints</summary>

```
GET    /                                   (redirect)
GET    /customers                          GET    /products                          GET    /orders
POST   /customers                          POST   /products                          POST   /orders
GET    /customers/new                      GET    /products/new                      GET    /orders/new
POST   /customers/page                     POST   /products/page                     POST   /orders/page
POST   /customers/selection                POST   /products/selection                POST   /orders/selection
POST   /customers/bulk-delete/confirm      POST   /products/bulk-delete/confirm      POST   /orders/bulk-delete/confirm
POST   /customers/bulk-delete              POST   /products/bulk-delete              POST   /orders/bulk-delete
GET    /customers/{customer_id}            GET    /products/{product_id}             GET    /orders/{order_id}
PUT    /customers/{customer_id}            PUT    /products/{product_id}             PUT    /orders/{order_id}
DELETE /customers/{customer_id}            DELETE /products/{product_id}             DELETE /orders/{order_id}
GET    /customers/{customer_id}/edit       GET    /products/{product_id}/edit        GET    /orders/{order_id}/edit
```
</details>

The **#** column links to each endpoint's diagram.

## Per-resource differences

| | Customers | Products | Orders |
|---|---|---|---|
| List order | `last_name, first_name, id` | `name, id` | `created_at DESC, id DESC` |
| Create / update validation | names required, email has `@`; duplicate email → 422 | name, SKU required; price 0 to $21,474,836.47; duplicate SKU → 422 | customer and every product must exist; ≥ 1 line item; prices snapshotted from products |
| New / edit form also loads | - | - | all customers and products (dropdowns) |
| Writes | one `INSERT` / `UPDATE` | one `INSERT` / `UPDATE` | header + items in one transaction (update replaces all items) |
| Delete guard | refused while orders reference the customer | refused while order items use the product | none (items cascade) |
| Bulk dialog label / detail | name / email | name / SKU | "Order #id" / customer name |
