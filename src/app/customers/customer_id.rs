//! `/customers/{customer_id}`: read one row, update, delete.

pub(crate) mod edit;

use topcoat::{
    Result,
    context::Cx,
    router::{StatusCode, content::Form, module_param, path_param, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{CustomerRow, customer_row};
use crate::{
    app::pool,
    customer::{models::CustomerForm, services as service},
};

module_param!(pub(crate) customer_id: i64, error = bad_request("Invalid customer id."));

pub(crate) fn id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<CustomerId>(cx)?)
}

/// `GET /customers/{id}` - the read-only row fragment.
#[route(GET)]
pub(crate) async fn show_row(cx: &Cx) -> Result<ViewHandle> {
    let customer = CustomerRow::from(service::get(pool(cx), id(cx)?).await?);
    view! { cx => customer_row(customer: &customer) }
        .single()
        .await
}

/// `PUT /customers/{id}` - saves, then returns the updated row to swap in.
#[route(PUT)]
pub(crate) async fn update(cx: &Cx, Form(form): Form<CustomerForm>) -> Result<ViewHandle> {
    let customer = CustomerRow::from(service::update(pool(cx), id(cx)?, form).await?);
    view! { cx => customer_row(customer: &customer) }
        .single()
        .await
}

/// `DELETE /customers/{id}` - an empty 200 makes htmx's `outerHTML` swap
/// remove the row.
#[route(DELETE)]
pub(crate) async fn delete(cx: &Cx) -> Result<StatusCode> {
    service::delete(pool(cx), id(cx)?).await?;
    Ok(StatusCode::OK)
}
