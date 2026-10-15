//! `/products`: the list page, and `POST` to create.
//!
//! Also home to the product `view!` components (row, rows, form), which the
//! child modules reuse for their fragment responses.

pub(crate) mod new;
pub(crate) mod product_id;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, page, route},
    view::{View, ViewExt, ViewHandle, class, component, view},
};

use super::pool;
use crate::{
    product::{
        models::{Product, ProductForm},
        services as service,
    },
    ui::{self, INPUT},
};
use product_id::ProductId;

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

fn to_rows(products: Vec<Product>) -> Vec<ProductRow> {
    products.into_iter().map(ProductRow::from).collect()
}

/// `GET /products`
#[page]
pub(crate) async fn index(cx: &Cx) -> Result<impl View> {
    let products = to_rows(service::list(pool(cx)).await?);
    let new_url = href!(new::new_form).resolve(cx);

    Ok(view! {
        ui::page_header(title: "Products", add_label: "Add product", new_url: &new_url)
        ui::table_card(
            ui::table_head(
                columns: &["Name", "SKU", "Price", "Description", "Added", ""]
            )
            <tbody id="rows-body">product_rows(products: products)</tbody>
        )
    })
}

/// `POST /products` - creates the product, then re-renders every row.
#[route(POST)]
pub(crate) async fn create(cx: &Cx, Form(form): Form<ProductForm>) -> Result<ViewHandle> {
    service::create(pool(cx), form).await?;
    let products = to_rows(service::list(pool(cx)).await?);
    view! { cx => product_rows(products: products) }
        .single()
        .await
}

#[component]
pub(crate) async fn product_rows(products: Vec<ProductRow>) -> Result<impl View> {
    Ok(view! {
        if products.is_empty() {
            ui::empty_rows(
                colspan: 6,
                title: "The catalog is empty.",
                hint: "Add your first product to get started."
            )
        } else {
            #[key(product.id)]
            for product in products {
                product_row(product: &product)
            }
        }
    })
}

/// One read-only table row, `product-row-<id>`.
#[component]
pub(crate) async fn product_row(cx: &Cx, product: &ProductRow) -> Result<impl View> {
    let row_id = format!("product-row-{}", product.id);
    let edit_url = href!(product_id::edit::edit_form, ProductId(product.id)).resolve(cx);
    let delete_url = href!(product_id::delete, ProductId(product.id)).resolve(cx);

    Ok(view! {
        <tr
            id=(&row_id)
            class="group border-b border-line text-ink last:border-0 transition-colors hover:bg-paper"
            title=(format!("Last updated {}", product.last_updated))
        >
            <td
                class="border-l-2 border-l-transparent py-3 pr-4 pl-[14px] font-medium transition-colors group-hover:border-l-brand"
            >
                (&product.name)
            </td>
            <td class="px-4 py-3 font-mono text-xs text-muted">(&product.sku)</td>
            <td class="px-4 py-3 font-mono text-xs tabular-nums text-muted">
                (&product.price_display)
            </td>
            <td
                class="max-w-[16rem] truncate px-4 py-3 text-muted"
                title=(product.description.as_deref())
            >
                (product.description.as_deref().unwrap_or("—"))
            </td>
            <td class="px-4 py-3 text-right font-mono text-xs tabular-nums text-muted">
                (&product.added_on)
            </td>
            ui::row_actions(
                label: &product.name,
                row_id: &row_id,
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
                    hx-target="#rows-body"
                    hx-swap="innerHTML"
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
