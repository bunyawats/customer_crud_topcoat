//! `/orders`: the list page, and `POST` to create.
//!
//! Also home to the order `view!` components (row, rows, form, line-item
//! row), which the child modules reuse for their fragment responses.

pub(crate) mod bulk_delete;
pub(crate) mod new;
pub(crate) mod order_id;
pub(crate) mod page;
pub(crate) mod selection;

use std::collections::HashSet;

use sqlx::PgPool;
use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, page, route},
    view::{View, ViewExt, ViewHandle, class, component, view},
};

use super::{clear_selection, pool, selected_ids};
use crate::{
    customer::services as customer_service,
    error::AppError,
    order::{
        models::{LineItemInput, OrderDetail, OrderForm, OrderSummary, STATUSES},
        services as service,
    },
    pagination::{Page, PageRequest},
    product::services as product_service,
    selection::models::Resource,
    ui::{self, ACTIONS_COLUMN, Column, PagerInfo, SELECT},
};
use order_id::OrderId;

/// Id of the list region; `#<LIST_ID>-state` holds its hidden page inputs.
pub(crate) const LIST_ID: &str = "order-list";
/// Which stored selection this list's checkboxes read and write.
pub(crate) const RESOURCE: Resource = Resource::Orders;
/// Id of the selection toolbar above the list (sent out-of-band on changes).
pub(crate) const TOOLBAR_ID: &str = "order-toolbar";
const LIST_TARGET: &str = "#order-list";
/// Columns after the selection box. Fixed widths for the short, regular
/// data; the customer name takes the rest.
const COLUMNS: &[Column] = &[
    Column {
        label: "Order",
        width: "w-24",
    },
    Column {
        label: "Customer",
        width: "",
    },
    Column {
        label: "Status",
        width: "w-32",
    },
    Column {
        label: "Total",
        width: "w-36",
    },
    Column {
        label: "Placed",
        width: "w-32",
    },
    ACTIONS_COLUMN,
];

/// Flat, owned view model for a single order row.
pub(crate) struct OrderRow {
    id: i64,
    customer_name: String,
    status: String,
    total_display: String,
    placed_on: String,
}

impl From<OrderSummary> for OrderRow {
    fn from(order: OrderSummary) -> Self {
        OrderRow {
            id: order.id,
            customer_name: order.customer_name,
            status: order.status,
            total_display: format!("${:.2}", order.total_cents as f64 / 100.0),
            placed_on: order.created_at.format("%Y-%m-%d").to_string(),
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `GET /orders`
#[page]
pub(crate) async fn index(cx: &Cx) -> Result<impl View> {
    clear_selection(cx, RESOURCE).await?;
    let selected = HashSet::new();
    let listing = service::list_page(pool(cx), PageRequest::first())
        .await?
        .map(OrderRow::from);
    let new_url = href!(new::new_form).resolve(cx);

    Ok(view! {
        ui::page_header(title: "Orders", add_label: "Add order", new_url: &new_url)
        order_toolbar(count: 0)
        order_list(listing: listing, selected: &selected)
    })
}

/// `POST /orders` - creates the order, then re-renders the list on the
/// page that holds it (so the new row is visible in its sorted position).
#[route(POST)]
pub(crate) async fn create(cx: &Cx, Form(form): Form<OrderForm>) -> Result<ViewHandle> {
    let id = service::create(pool(cx), form).await?;
    let page = service::page_of(pool(cx), id).await?;
    let listing = service::list_page(pool(cx), PageRequest::new(page, ""))
        .await?
        .map(OrderRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! { cx => order_list(listing: listing, selected: &selected) }
        .single()
        .await
}

/// The `#order-list` region: hidden list state, the table and its pager.
/// Prev/Next, create and delete all replace it whole (`outerHTML`).
#[component]
pub(crate) async fn order_list(
    cx: &Cx,
    listing: Page<OrderRow>,
    selected: &HashSet<i64>,
    #[default] oob: bool,
) -> Result<impl View> {
    let page_url = href!(page::show_page).resolve(cx);
    let select_url = href!(selection::update_selection).resolve(cx);
    let page_ids: Vec<i64> = listing.items.iter().map(|row| row.id).collect();
    let all_selected = !page_ids.is_empty() && page_ids.iter().all(|id| selected.contains(id));
    let pager = PagerInfo::from(&listing);
    let orders = listing.items;
    Ok(view! {
        <div id=(LIST_ID) hx-swap-oob=(oob.then_some("true"))>
            ui::list_state(
                list_id: LIST_ID,
                page: pager.page,
                query_id: &pager.query_id
            )
            ui::table_card(
                ui::table_head(
                    columns: COLUMNS,
                    ui::select_all_checkbox(
                        ids: &page_ids,
                        checked: all_selected,
                        select_url: &select_url,
                        list_id: LIST_ID
                    )
                )
                <tbody id="rows-body">
                    order_rows(orders: orders, selected: selected)
                </tbody>
                ui::pager(colspan: 7, url: &page_url, list_id: LIST_ID, info: &pager)
            )
        </div>
    })
}

/// This list's selection toolbar (see [`ui::bulk_toolbar`]).
#[component]
pub(crate) async fn order_toolbar(
    cx: &Cx,
    count: usize,
    #[default] oob: bool,
) -> Result<impl View> {
    let confirm_url = href!(bulk_delete::confirm::confirm_bulk_delete).resolve(cx);
    Ok(view! {
        ui::bulk_toolbar(
            toolbar_id: TOOLBAR_ID,
            count: count,
            confirm_url: &confirm_url,
            list_id: LIST_ID,
            oob: oob
        )
    })
}

/// Fresh display fields for the selected ids, for the bulk dialogs. Ids
/// that no longer exist (deleted elsewhere) are dropped.
pub(crate) async fn bulk_items(cx: &Cx, ids: &[i64]) -> Result<Vec<ui::BulkItem>> {
    let mut items = Vec::with_capacity(ids.len());
    for &id in ids {
        match service::get(pool(cx), id).await {
            Ok(detail) => items.push(ui::BulkItem {
                id,
                label: format!("Order #{id}"),
                detail: OrderRow::from(detail.order).customer_name,
            }),
            Err(AppError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(items)
}

#[component]
pub(crate) async fn order_rows(
    orders: Vec<OrderRow>,
    selected: &HashSet<i64>,
) -> Result<impl View> {
    Ok(view! {
        if orders.is_empty() {
            ui::empty_rows(
                colspan: 7,
                title: "The ledger is empty.",
                hint: "Add your first order to get started."
            )
        } else {
            #[key(order.id)]
            for order in orders {
                order_row(order: &order, selected: selected.contains(&order.id))
            }
        }
    })
}

/// One read-only table row, `order-row-<id>`.
#[component]
pub(crate) async fn order_row(cx: &Cx, order: &OrderRow, selected: bool) -> Result<impl View> {
    let select_url = href!(selection::update_selection).resolve(cx);
    let row_id = format!("order-row-{}", order.id);
    let edit_url = href!(order_id::edit::edit_form, OrderId(order.id)).resolve(cx);
    let delete_url = href!(order_id::delete, OrderId(order.id)).resolve(cx);
    let label = format!("order {}", order.id);
    let tone = match order.status.as_str() {
        "cancelled" => "bg-danger-tint text-danger",
        "pending" => "bg-paper text-muted",
        _ => "bg-brand-tint text-brand",
    };

    Ok(view! {
        <tr
            id=(&row_id)
            class="group border-b border-line text-ink last:border-0 transition-colors hover:bg-paper has-checked:bg-brand-tint/40"
        >
            ui::row_checkbox(
                id: order.id,
                label: &label,
                checked: selected,
                select_url: &select_url,
                list_id: LIST_ID
            )
            <td class="px-4 py-3 font-mono text-xs text-muted">
                "#"
                (order.id)
            </td>
            <td class="truncate px-4 py-3 font-medium" title=(&order.customer_name)>
                (&order.customer_name)
            </td>
            <td class="px-4 py-3">
                <span
                    class=(class!(
                        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                        tone,
                    ))
                >
                    (capitalize(&order.status))
                </span>
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&order.total_display)
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&order.placed_on)
            </td>
            ui::row_actions(
                label: &label,
                list_id: LIST_ID,
                edit_url: &edit_url,
                delete_url: &delete_url,
                confirm: "Delete this order?"
            )
        </tr>
    })
}

// --- Form -------------------------------------------------------------------

/// An `<option>` for a customer or product `<select>`.
pub(crate) struct Choice {
    id: i64,
    label: String,
}

/// Everything the order form renders: the choices to pick from, plus the
/// current values when editing (`order_id: Some`).
pub(crate) struct OrderFormData {
    order_id: Option<i64>,
    customer_id: Option<i64>,
    status: String,
    items: Vec<LineItemInput>,
    customers: Vec<Choice>,
    products: Vec<Choice>,
}

impl OrderFormData {
    /// Loads the customer and product choices, pre-filled from `detail` when
    /// editing an existing order.
    pub(crate) async fn load(pool: &PgPool, detail: Option<OrderDetail>) -> Result<Self, AppError> {
        let customers = customer_service::list(pool)
            .await?
            .into_iter()
            .map(|c| Choice {
                id: c.id,
                label: format!("{} {}", c.first_name, c.last_name),
            })
            .collect();
        let products = product_service::list(pool)
            .await?
            .into_iter()
            .map(|p| Choice {
                id: p.id,
                label: format!("{} — ${:.2}", p.name, p.price_cents as f64 / 100.0),
            })
            .collect();

        Ok(match detail {
            None => OrderFormData {
                order_id: None,
                customer_id: None,
                status: "pending".to_string(),
                items: Vec::new(),
                customers,
                products,
            },
            Some(detail) => OrderFormData {
                order_id: Some(detail.order.id),
                customer_id: Some(detail.order.customer_id),
                status: detail.order.status,
                items: detail
                    .items
                    .iter()
                    .map(|item| LineItemInput {
                        product_id: item.product_id,
                        quantity: item.quantity,
                    })
                    .collect(),
                customers,
                products,
            },
        })
    }
}

/// The create or edit form. Line items are client-side rows (see
/// `static/app.js`) mirrored into the hidden `items_json` field, because a
/// plain form can't submit aligned (product, quantity) arrays.
#[component]
pub(crate) async fn order_form(cx: &Cx, data: &OrderFormData) -> Result<impl View> {
    let (form_id, title, subtitle, submit) = match data.order_id {
        None => (
            "create-order-form",
            "Add order",
            "Create a new record in the ledger.",
            "Add order",
        ),
        Some(_) => (
            "edit-order-form",
            "Edit order",
            "Update this record in the ledger.",
            "Save changes",
        ),
    };
    let target = match data.order_id {
        None => href!(create).resolve(cx),
        Some(id) => href!(order_id::update, OrderId(id)).resolve(cx),
    };
    let items_json = serde_json::to_string(&data.items)?;

    Ok(view! {
        <form
            id=(form_id)
            match data.order_id {
                None => {
                    hx-post=(&target)
                    hx-target=(LIST_TARGET)
                    hx-swap="outerHTML"
                }
                Some(id) => {
                    hx-put=(&target)
                    hx-target=(format!("#order-row-{id}"))
                    hx-swap="outerHTML"
                }
            }
        >
            ui::form_header(title: title, subtitle: subtitle)

            <div class="mt-5 space-y-4">
                ui::field(
                    label: "Customer",
                    <select name="customer_id" required="" autofocus="" class=(SELECT)>
                        <option value="">"Select a customer…"</option>
                        for c in &data.customers {
                            <option
                                value=(c.id)
                                selected=(Some(c.id) == data.customer_id)
                            >
                                (&c.label)
                            </option>
                        }
                    </select>
                )
                ui::field(
                    label: "Status",
                    <select name="status" class=(SELECT)>
                        for status in STATUSES {
                            <option value=(status) selected=(status == data.status)>
                                (capitalize(status))
                            </option>
                        }
                    </select>
                )

                <div>
                    <span
                        class="text-xs font-medium uppercase tracking-wide text-muted"
                    >
                        "Items"
                    </span>
                    <div class="mt-1 space-y-2" data-line-items="">
                        if data.items.is_empty() {
                            line_item_row(products: &data.products)
                        } else {
                            #[key(position)]
                            for (position, item) in data.items.iter().enumerate() {
                                line_item_row(
                                    products: &data.products,
                                    selected: Some(item.product_id),
                                    quantity: item.quantity
                                )
                            }
                        }
                    </div>
                    <button
                        type="button"
                        data-add-line-item=""
                        class="mt-2 inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-brand transition-colors hover:bg-brand-tint"
                    >
                        ui::icon(kind: ui::Icon::Plus, size: 14)
                        "Add item"
                    </button>
                </div>
            </div>

            <input type="hidden" name="items_json" value=(items_json)>

            // Cloned by app.js for "Add item".
            <template data-line-item-template="">
                line_item_row(products: &data.products)
            </template>

            ui::form_actions(submit_label: submit)
        </form>
    })
}

/// One (product, quantity) row. Its controls deliberately have no `name`:
/// only `items_json` is submitted.
#[component]
async fn line_item_row(
    products: &[Choice],
    #[default] selected: Option<i64>,
    #[default(1)] quantity: i32,
) -> Result<impl View> {
    Ok(view! {
        <div class="line-item-row flex items-end gap-2" data-line-item="">
            ui::field(
                label: "Product",
                class: Some("flex-1"),
                <select data-field="product_id" required="" class=(SELECT)>
                    <option value="">"Select a product…"</option>
                    for p in products {
                        <option value=(p.id) selected=(Some(p.id) == selected)>
                            (&p.label)
                        </option>
                    }
                </select>
            )
            ui::field(
                label: "Qty",
                class: Some("w-24"),
                <input
                    type="number"
                    data-field="quantity"
                    min="1"
                    value=(quantity)
                    required=""
                    class=(SELECT)
                >
            )
            <button
                type="button"
                data-remove-row=""
                aria-label="Remove line item"
                class="mb-0.5 rounded-md p-2 text-muted transition-colors hover:bg-danger-tint hover:text-danger"
            >
                ui::icon(kind: ui::Icon::X)
            </button>
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::capitalize;

    #[test]
    fn capitalizes_first_letter_only() {
        assert_eq!(capitalize("pending"), "Pending");
        assert_eq!(capitalize(""), "");
    }
}
