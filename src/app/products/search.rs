//! `POST /products/search`: runs a search and stores it as this browser's
//! filter for the product list.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{ProductRow, RESOURCE, product_list, product_search};
use crate::{
    app::{SearchForm, pool, save_filter, selected_ids},
    pagination::PageRequest,
    product::services as service,
};

/// Validates and stores the criteria (an empty `q` clears them), then
/// returns the list's first page under them with a fresh count. Ticks are
/// kept. Clearing also re-renders the search box empty, out-of-band.
#[route(POST)]
pub(crate) async fn run_search(cx: &Cx, Form(form): Form<SearchForm>) -> Result<ViewHandle> {
    let filter = form.filter()?;
    save_filter(cx, RESOURCE, &filter).await?;
    let listing = service::list_page(pool(cx), PageRequest::first(), &filter)
        .await?
        .map(ProductRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        product_list(listing: listing, filter: &filter, selected: &selected)
        if filter.is_empty() {
            product_search(value: "", oob: true)
        }
    }
    .single()
    .await
}
