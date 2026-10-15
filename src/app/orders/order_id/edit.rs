//! `/orders/{order_id}/edit`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use crate::{
    app::{
        orders::{OrderFormData, order_form},
        pool,
    },
    order::services as service,
};

/// `GET /orders/{id}/edit` - the pre-filled edit form, loaded into the
/// dialog.
#[route(GET)]
pub(crate) async fn edit_form(cx: &Cx) -> Result<ViewHandle> {
    let detail = service::get(pool(cx), super::id(cx)?).await?;
    let data = OrderFormData::load(pool(cx), Some(detail)).await?;
    view! { cx => order_form(data: &data) }.single().await
}
