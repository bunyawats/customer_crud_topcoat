//! `/products/new`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use super::product_form;

/// `GET /products/new` - the empty create form, loaded into the dialog.
#[route(GET)]
pub(crate) async fn new_form(cx: &Cx) -> Result<ViewHandle> {
    view! { cx => product_form() }.single().await
}
