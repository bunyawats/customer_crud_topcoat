//! `/products`: the list page, and `POST` to create.
//!
//! Also home to the product `view!` components (row, rows, form), which the
//! child modules reuse for their fragment responses.

pub(crate) mod bulk_delete;
pub(crate) mod new;
pub(crate) mod page;
pub(crate) mod product_id;
pub(crate) mod selection;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, page, route},
    view::{View, ViewExt, ViewHandle, class, component, view},
};

use std::collections::HashSet;

use super::{clear_selection, pool, selected_ids};
use crate::{
    error::AppError,
    pagination::{Page, PageRequest},
    product::{
        models::{Product, ProductForm},
        services as service,
    },
    selection::models::Resource,
    ui::{self, ACTIONS_COLUMN, Column, INPUT, PagerInfo},
};
use product_id::ProductId;

/// Id of the list region; `#<LIST_ID>-state` holds its hidden page inputs.
pub(crate) const LIST_ID: &str = "product-list";
/// Which stored selection this list's checkboxes read and write.
pub(crate) const RESOURCE: Resource = Resource::Products;
/// Id of the selection toolbar above the list (sent out-of-band on changes).
pub(crate) const TOOLBAR_ID: &str = "product-toolbar";
const LIST_TARGET: &str = "#product-list";
/// Columns after the selection box. Fixed widths for the short, regular
/// data; name and description share the rest.
const COLUMNS: &[Column] = &[
    Column {
        label: "Name",
        width: "",
    },
    Column {
        label: "SKU",
        width: "w-32",
    },
    Column {
        label: "Price",
        width: "w-32",
    },
    Column {
        label: "Description",
        width: "",
    },
    Column {
        label: "Added",
        width: "w-32",
    },
    ACTIONS_COLUMN,
];

/// Flat, owned view model for a single table row.
pub(crate) struct ProductRow {
    id: i64,
    name: String,
    sku: String,
    /// Formatted for display, e.g. "$19.99".
    price_display: String,
    /// Plain decimal, e.g. "19.99" - for the `type="number"` input's value,
    /// which `price_display`'s "$" prefix would break.
    price_input: String,
    description: Option<String>,
    added_on: String,
    last_updated: String,
}

impl From<Product> for ProductRow {
    fn from(product: Product) -> Self {
        let dollars = product.price_cents as f64 / 100.0;
        ProductRow {
            id: product.id,
            name: product.name,
            sku: product.sku,
            price_display: format!("${dollars:.2}"),
            price_input: format!("{dollars:.2}"),
            description: product.description,
            added_on: product.created_at.format("%Y-%m-%d").to_string(),
            last_updated: product.updated_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        }
    }
}

/// `GET /products`
#[page]
pub(crate) async fn index(cx: &Cx) -> Result<impl View> {
    clear_selection(cx, RESOURCE).await?;
    let selected = HashSet::new();
    let listing = service::list_page(pool(cx), PageRequest::first())
        .await?
        .map(ProductRow::from);
    let new_url = href!(new::new_form).resolve(cx);

    Ok(view! {
        ui::page_header(title: "Products", add_label: "Add product", new_url: &new_url)
        product_toolbar(count: 0)
        product_list(listing: listing, selected: &selected)
    })
}

/// `POST /products` - creates the product, then re-renders the list on the
/// page that holds it (so the new row is visible in its sorted position).
#[route(POST)]
pub(crate) async fn create(cx: &Cx, Form(form): Form<ProductForm>) -> Result<ViewHandle> {
    let id = service::create(pool(cx), form).await?.id;
    let page = service::page_of(pool(cx), id).await?;
    let listing = service::list_page(pool(cx), PageRequest::new(page, ""))
        .await?
        .map(ProductRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! { cx => product_list(listing: listing, selected: &selected) }
        .single()
        .await
}

/// The `#product-list` region: hidden list state, the table and its pager.
/// Prev/Next, create and delete all replace it whole (`outerHTML`).
#[component]
pub(crate) async fn product_list(
    cx: &Cx,
    listing: Page<ProductRow>,
    selected: &HashSet<i64>,
    #[default] oob: bool,
) -> Result<impl View> {
    let page_url = href!(page::show_page).resolve(cx);
    let select_url = href!(selection::update_selection).resolve(cx);
    let page_ids: Vec<i64> = listing.items.iter().map(|row| row.id).collect();
    let all_selected = !page_ids.is_empty() && page_ids.iter().all(|id| selected.contains(id));
    let pager = PagerInfo::from(&listing);
    let products = listing.items;
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
                    product_rows(products: products, selected: selected)
                </tbody>
                ui::pager(colspan: 7, url: &page_url, list_id: LIST_ID, info: &pager)
            )
        </div>
    })
}

/// This list's selection toolbar (see [`ui::bulk_toolbar`]).
#[component]
pub(crate) async fn product_toolbar(
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
            Ok(product) => items.push(ui::BulkItem {
                id,
                label: product.name,
                detail: product.sku,
            }),
            Err(AppError::NotFound(_)) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(items)
}

#[component]
pub(crate) async fn product_rows(
    products: Vec<ProductRow>,
    selected: &HashSet<i64>,
) -> Result<impl View> {
    Ok(view! {
        if products.is_empty() {
            ui::empty_rows(
                colspan: 7,
                title: "The catalog is empty.",
                hint: "Add your first product to get started."
            )
        } else {
            #[key(product.id)]
            for product in products {
                product_row(product: &product, selected: selected.contains(&product.id))
            }
        }
    })
}

/// One read-only table row, `product-row-<id>`.
#[component]
pub(crate) async fn product_row(
    cx: &Cx,
    product: &ProductRow,
    selected: bool,
) -> Result<impl View> {
    let select_url = href!(selection::update_selection).resolve(cx);
    let row_id = format!("product-row-{}", product.id);
    let edit_url = href!(product_id::edit::edit_form, ProductId(product.id)).resolve(cx);
    let delete_url = href!(product_id::delete, ProductId(product.id)).resolve(cx);

    Ok(view! {
        <tr
            id=(&row_id)
            class="group border-b border-line text-ink last:border-0 transition-colors hover:bg-paper has-checked:bg-brand-tint/40"
            title=(format!("Last updated {}", product.last_updated))
        >
            ui::row_checkbox(
                id: product.id,
                label: &product.name,
                checked: selected,
                select_url: &select_url,
                list_id: LIST_ID
            )
            <td class="truncate px-4 py-3 font-medium" title=(&product.name)>
                (&product.name)
            </td>
            <td
                class="truncate px-4 py-3 font-mono text-xs text-muted"
                title=(&product.sku)
            >
                (&product.sku)
            </td>
            <td class="px-4 py-3 font-mono text-xs tabular-nums text-muted">
                (&product.price_display)
            </td>
            <td
                class="truncate px-4 py-3 text-muted"
                title=(product.description.as_deref())
            >
                (product.description.as_deref().unwrap_or("—"))
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&product.added_on)
            </td>
            ui::row_actions(
                label: &product.name,
                list_id: LIST_ID,
                edit_url: &edit_url,
                delete_url: &delete_url,
                confirm: "Delete this product?"
            )
        </tr>
    })
}

/// The create form (`existing: None`) or the edit form for one product.
#[component]
pub(crate) async fn product_form(
    cx: &Cx,
    #[default] existing: Option<&ProductRow>,
) -> Result<impl View> {
    let (form_id, title, subtitle, submit) = match existing {
        None => (
            "create-product-form",
            "Add product",
            "Create a new record in the catalog.",
            "Add product",
        ),
        Some(_) => (
            "edit-product-form",
            "Edit product",
            "Update this record in the catalog.",
            "Save changes",
        ),
    };
    let target = match existing {
        None => href!(create).resolve(cx),
        Some(product) => href!(product_id::update, ProductId(product.id)).resolve(cx),
    };

    Ok(view! {
        <form
            id=(form_id)
            match existing {
                None => {
                    hx-post=(&target)
                    hx-target=(LIST_TARGET)
                    hx-swap="outerHTML"
                }
                Some(product) => {
                    hx-put=(&target)
                    hx-target=(format!("#product-row-{}", product.id))
                    hx-swap="outerHTML"
                }
            }
        >
            ui::form_header(title: title, subtitle: subtitle)

            <div class="mt-5 space-y-4">
                ui::field(
                    label: "Name",
                    <input
                        type="text"
                        name="name"
                        value=(existing.map(|p| p.name.as_str()))
                        required=""
                        autofocus=""
                        class=(INPUT)
                    >
                )
                ui::field(
                    label: "SKU",
                    <input
                        type="text"
                        name="sku"
                        value=(existing.map(|p| p.sku.as_str()))
                        required=""
                        class=(class!(INPUT, "font-mono"))
                    >
                )
                ui::field(
                    label: "Price (USD)",
                    <input
                        type="number"
                        name="price"
                        step="0.01"
                        min="0"
                        placeholder="0.00"
                        value=(existing.map(|p| p.price_input.as_str()))
                        required=""
                        class=(class!(INPUT, "font-mono"))
                    >
                )
                ui::field(
                    label: "Description",
                    <textarea name="description" rows="3" class=(INPUT)>
                        (existing.and_then(|p| p.description.as_deref()).unwrap_or(""))
                    </textarea>
                )
            </div>

            ui::form_actions(submit_label: submit)
        </form>
    })
}
