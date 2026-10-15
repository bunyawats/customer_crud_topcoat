//! `/orders/{order_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{OrderRow, RESOURCE, order_list, order_row, order_toolbar};
use crate::{
    app::{list_filter, page_after_write, pool, selected_ids},
    order::{models::OrderForm, services as service},
    selection::services as selection,
};

module_param!(pub(crate) order_id: i64, error = bad_request("Invalid order id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<OrderId>(cx)?)
}

/// `GET /orders/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let order = OrderRow::from(service::get(pool(cx), id(cx)?).await?.order);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&order.id);
    view! { cx => order_row(order: &order, selected: selected) }
        .single()
        .await
}

/// `PUT /orders/{id}` - saves the header and replaces the line items, then
/// returns the updated row (with its recomputed total) to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<OrderForm>) -> Result<ViewHandle> {
    let id = id(cx)?;
    service::update(pool(cx), id, form).await?;
    let order = OrderRow::from(service::get(pool(cx), id).await?.order);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&order.id);
    view! { cx => order_row(order: &order, selected: selected) }
        .single()
        .await
}

/// `DELETE /orders/{id}` - then re-renders the list on the page the user
/// was on (`page` arrives in the query string via the button's `hx-include`).
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<ViewHandle> {
    let id = id(cx)?;
    service::delete(pool(cx), id).await?;
    selection::forget_item(pool(cx), RESOURCE, id).await?;
    let filter = list_filter(cx, RESOURCE).await?;
    let listing = service::list_page(pool(cx), page_after_write(cx)?, &filter)
        .await?
        .map(OrderRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        order_list(listing: listing, filter: &filter, selected: &selected)
        order_toolbar(count: selected.len(), oob: true)
    }
    .single()
    .await
}
