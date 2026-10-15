//! `/customers/new`

use topcoat::{
    Result,
    context::Cx,
    router::route,
    view::{ViewExt, ViewHandle, view},
};

use super::customer_form;

/// `GET /customers/new` - the empty create form, loaded into the dialog.
#[route(GET)]
pub(crate) async fn new_form(cx: &Cx) -> Result<ViewHandle> {
    view! { cx => customer_form() }.single().await
}
