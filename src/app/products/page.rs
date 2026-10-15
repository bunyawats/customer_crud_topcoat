//! `POST /products/page`: Prev/Next on the product list.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{ProductRow, RESOURCE, product_list};
use crate::{
    app::{PageForm, list_filter, pool, selected_ids},
    product::services as service,
};

/// Re-renders the list region for the requested page, reusing the cached
/// total while the posted `query_id` is still valid.
#[route(POST)]
pub(crate) async fn show_page(cx: &Cx, Form(form): Form<PageForm>) -> Result<ViewHandle> {
    let filter = list_filter(cx, RESOURCE).await?;
    let listing = service::list_page(pool(cx), form.request(), &filter)
        .await?
        .map(ProductRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! { cx => product_list(listing: listing, filter: &filter, selected: &selected) }
        .single()
        .await
}
