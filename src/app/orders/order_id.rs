//! `/orders/{order_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{OrderRow, order_row};
use crate::{
    app::pool,
    order::{models::OrderForm, services as service},
};

module_param!(pub(crate) order_id: i64, error = bad_request("Invalid order id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<OrderId>(cx)?)
}

/// `GET /orders/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let order = OrderRow::from(service::get(pool(cx), id(cx)?).await?.order);
    view! { cx => order_row(order: &order) }.single().await
}

/// `PUT /orders/{id}` - saves the header and replaces the line items, then
/// returns the updated row (with its recomputed total) to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<OrderForm>) -> Result<ViewHandle> {
    let id = id(cx)?;
    service::update(pool(cx), id, form).await?;
    let order = OrderRow::from(service::get(pool(cx), id).await?.order);
    view! { cx => order_row(order: &order) }.single().await
}

/// `DELETE /orders/{id}` - line items go with it (`ON DELETE CASCADE`).
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<StatusCode> {
    service::delete(pool(cx), id(cx)?).await?;
    Ok(StatusCode::OK)
}
