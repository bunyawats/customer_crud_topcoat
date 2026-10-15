//! `/products/{product_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{ProductRow, product_row};
use crate::{
    app::pool,
    product::{models::ProductForm, services as service},
};

module_param!(pub(crate) product_id: i64, error = bad_request("Invalid product id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<ProductId>(cx)?)
}

/// `GET /products/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let product = ProductRow::from(service::get(pool(cx), id(cx)?).await?);
    view! { cx => product_row(product: &product) }
        .single()
        .await
}

/// `PUT /products/{id}` - saves, then returns the updated row to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<ProductForm>) -> Result<ViewHandle> {
    let product = ProductRow::from(service::update(pool(cx), id(cx)?, form).await?);
    view! { cx => product_row(product: &product) }
        .single()
        .await
}

/// `DELETE /products/{id}` - an empty 200 makes htmx's `outerHTML` swap
/// remove the row.
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<StatusCode> {
    service::delete(pool(cx), id(cx)?).await?;
    Ok(StatusCode::OK)
}
