//! `POST /orders/search`: runs a search and stores it as this browser's
//! filter for the order list.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{OrderRow, RESOURCE, order_list, order_search};
use crate::{
    app::{SearchForm, pool, save_filter, selected_ids},
    order::services as service,
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
        .map(OrderRow::from);
    let selected = selected_ids(cx, RESOURCE).await?;
    view! {
        cx =>
        order_list(listing: listing, filter: &filter, selected: &selected)
        if filter.is_empty() {
            order_search(value: "", oob: true)
        }
    }
    .single()
    .await
}
