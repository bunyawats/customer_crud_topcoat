//! `/orders/new`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use super::{OrderFormData, order_form};
use crate::app::pool;

/// `GET /orders/new` - the empty create form, loaded into the dialog.
#[route(GET)]
pub(crate) async fn new_form(cx: &Cx) -> Result<ViewHandle> {
    let data = OrderFormData::load(pool(cx), None).await?;
    view! { cx => order_form(data: &data) }.single().await
}
