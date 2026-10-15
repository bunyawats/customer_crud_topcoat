//! `/customers/{customer_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{CustomerRow, RESOURCE, customer_list, customer_row, customer_toolbar};
use crate::{
    app::{page_after_write, pool, selected_ids},
    customer::{models::CustomerForm, services as service},
    selection::services as selection,
};

module_param!(pub(crate) customer_id: i64, error = bad_request("Invalid customer id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<CustomerId>(cx)?)
}

/// `GET /customers/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let customer = CustomerRow::from(service::get(pool(cx), id(cx)?).await?);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&customer.id);
    view! { cx => customer_row(customer: &customer, selected: selected) }
        .single()
        .await
}

/// `PUT /customers/{id}` - saves, then returns the updated row to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<CustomerForm>) -> Result<ViewHandle> {
    let customer = CustomerRow::from(service::update(pool(cx), id(cx)?, form).await?);
    let selected = selected_ids(cx, RESOURCE).await?.contains(&customer.id);
    view! { cx => customer_row(customer: &customer, selected: selected) }
        .single()
        .await
}

/// `DELETE /customers/{id}` - then re-renders the list on the page the user
/// was on (`page` arrives in the query string via the button's `hx-include`).
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<ViewHandle> {
    let id = id(cx)?;
    service::delete(pool(cx), id).await?;
    selection::forget_item(pool(cx), RESOURCE, id).await?;
    let listing = service::list_page(pool(cx), page_after_write(cx)?)
        .await?
        .map(CustomerRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        customer_list(listing: listing, selected: &selected)
        customer_toolbar(count: selected.len(), oob: true)
    }
    .single()
    .await
}
