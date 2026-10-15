//! `POST /products/bulk-delete`: deletes every selected product.

pub(crate) mod confirm;

use std::collections::{HashMap, HashSet};

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, route},
    view::{ViewExt, ViewHandle, view},
};

use super::{ProductRow, RESOURCE, bulk_items, product_list, product_toolbar};
use crate::{
    app::{PageForm, list_filter, pool, session_key},
    product::services as service,
    selection::services as selection,
    ui::{self, BulkFailure},
};

/// Re-reads the selection server-side (never ids from the client), deletes
/// each through the single-item delete, clears the selection whatever the
/// outcome, and answers with the result dialog plus the refreshed list and
/// toolbar out-of-band.
#[route(POST)]
pub(crate) async fn execute_bulk_delete(cx: &Cx, Form(form): Form<PageForm>) -> Result<ViewHandle> {
    let key = session_key(cx).await?;
    let ids = match &key {
        Some(key) => selection::get(pool(cx), key, RESOURCE).await?,
        None => Vec::new(),
    };
    // Labels first: the deleted rows are gone afterwards.
    let labels: HashMap<i64, String> = bulk_items(cx, &ids)
        .await?
        .into_iter()
        .map(|item| (item.id, item.label))
        .collect();

    let results = service::bulk_delete(pool(cx), &ids).await?;

    if let Some(key) = &key {
        selection::clear(pool(cx), key, RESOURCE).await?;
    }
    for result in results.iter().filter(|result| result.ok()) {
        selection::forget_item(pool(cx), RESOURCE, result.id).await?;
    }

    let deleted = results.iter().filter(|result| result.ok()).count();
    let failures: Vec<BulkFailure> = results
        .iter()
        .filter_map(|result| {
            let error = result.error.clone()?;
            let label = labels
                .get(&result.id)
                .cloned()
                .unwrap_or_else(|| format!("#{}", result.id));
            Some(BulkFailure { label, error })
        })
        .collect();

    let filter = list_filter(cx, RESOURCE).await?;
    let listing = service::list_page(pool(cx), form.after_write(), &filter)
        .await?
        .map(ProductRow::from);
    let selected = HashSet::new();
    view! {
        cx =>
        ui::bulk_result_dialog(
            one: "product",
            many: "products",
            deleted: deleted,
            failures: &failures
        )
        product_list(listing: listing, filter: &filter, selected: &selected, oob: true)
        product_toolbar(count: 0, oob: true)
    }
    .single()
    .await
}
