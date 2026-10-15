//! `/products/{product_id}/edit`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use crate::{
    app::{
        pool,
        products::{ProductRow, product_form},
    },
    product::services as service,
};

/// `GET /products/{id}/edit` - the pre-filled edit form, loaded into the
/// dialog.
#[route(GET)]
pub(crate) async fn edit_form(cx: &Cx) -> Result<ViewHandle> {
    let product = ProductRow::from(service::get(pool(cx), super::id(cx)?).await?);
    view! { cx => product_form(existing: Some(&product)) }
        .single()
        .await
}
