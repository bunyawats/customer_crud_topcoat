//! `POST /customers/selection`: ticking a row's checkbox, or "select all on
//! this page".

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{CustomerRow, RESOURCE, customer_list, customer_toolbar};
use crate::{
    app::{SelectionForm, apply_selection, pool},
    customer::services as service,
};

/// Records the change server-side, then returns the list for the page the
/// user is on (swapped in by "select all"; ignored via `hx-swap="none"` by a
/// row checkbox) and the toolbar out-of-band, so the count stays right.
#[route(POST)]
pub(crate) async fn update_selection(
    cx: &Cx,
    Form(form): Form<SelectionForm>,
) -> Result<ViewHandle> {
    let selected = apply_selection(cx, RESOURCE, &form).await?;
    let listing = service::list_page(pool(cx), form.page_request())
        .await?
        .map(CustomerRow::from);
    view! {
        cx =>
        customer_list(listing: listing, selected: &selected)
        customer_toolbar(count: selected.len(), oob: true)
    }
    .single()
    .await
}
