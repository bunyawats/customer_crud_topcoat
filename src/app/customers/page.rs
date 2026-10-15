//! `POST /customers/page`: Prev/Next on the customer list.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{CustomerRow, RESOURCE, customer_list};
use crate::{
    app::{PageForm, pool, selected_ids},
    customer::services as service,
};

/// Re-renders the list region for the requested page, reusing the cached
/// total while the posted `query_id` is still valid.
#[route(POST)]
pub(crate) async fn show_page(cx: &Cx, Form(form): Form<PageForm>) -> Result<ViewHandle> {
    let listing = service::list_page(pool(cx), form.request())
        .await?
        .map(CustomerRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! { cx => customer_list(listing: listing, selected: &selected) }
        .single()
        .await
}
