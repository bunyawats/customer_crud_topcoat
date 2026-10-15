//! `/customers/{customer_id}/edit`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use crate::{
    app::{
        customers::{CustomerRow, customer_form},
        pool,
    },
    customer::services as service,
};

/// `GET /customers/{id}/edit` - the pre-filled edit form, loaded into the
/// dialog.
#[route(GET)]
pub(crate) async fn edit_form(cx: &Cx) -> Result<ViewHandle> {
    let customer = CustomerRow::from(service::get(pool(cx), super::id(cx)?).await?);
    view! { cx => customer_form(existing: Some(&customer)) }
        .single()
        .await
}
