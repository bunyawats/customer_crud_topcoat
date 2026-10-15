//! `/products/{product_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{ProductRow, RESOURCE, product_list, product_row, product_toolbar};
use crate::{
    app::{page_after_write, pool, selected_ids},
    product::{models::ProductForm, services as service},
    selection::services as selection,
};

module_param!(pub(crate) product_id: i64, error = bad_request("Invalid product id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<ProductId>(cx)?)
}

/// `GET /products/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let product = ProductRow::from(service::get(pool(cx), id(cx)?).await?);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&product.id);
    view! { cx => product_row(product: &product, selected: selected) }
        .single()
        .await
}

/// `PUT /products/{id}` - saves, then returns the updated row to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<ProductForm>) -> Result<ViewHandle> {
    let product = ProductRow::from(service::update(pool(cx), id(cx)?, form).await?);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&product.id);
    view! { cx => product_row(product: &product, selected: selected) }
        .single()
        .await
}

/// `DELETE /products/{id}` - then re-renders the list on the page the user
/// was on (`page` arrives in the query string via the button's `hx-include`).
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<ViewHandle> {
    let id = id(cx)?;
    service::delete(pool(cx), id).await?;
    selection::forget_item(pool(cx), RESOURCE, id).await?;
    let listing = service::list_page(pool(cx), page_after_write(cx)?)
        .await?
        .map(ProductRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        product_list(listing: listing, selected: &selected)
        product_toolbar(count: selected.len(), oob: true)
    }
    .single()
    .await
}
