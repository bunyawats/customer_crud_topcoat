//! `POST /customers/search`: runs a search and stores it as this browser's
//! filter for the customer list.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{CustomerRow, RESOURCE, customer_list, customer_search};
use crate::{
    app::{SearchForm, pool, save_filter, selected_ids},
    customer::services as service,
    pagination::PageRequest,
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
        .map(CustomerRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        customer_list(listing: listing, filter: &filter, selected: &selected)
        if filter.is_empty() {
            customer_search(value: "", oob: true)
        }
    }
    .single()
    .await
}
