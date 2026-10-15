//! `POST /orders/bulk-delete/confirm`: the dialog that asks before a bulk
//! delete.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, route},
    view::{ViewExt, ViewHandle, view},
};

use super::super::{RESOURCE, bulk_items};
use crate::{
    app::{PageForm, pool, session_key},
    bulk,
    selection::services as selection,
    ui,
};

/// Lists the selected orders with fresh names (stale ids dropped), or
/// explains why nothing can be deleted (empty, or over the batch cap).
#[route(POST)]
pub(crate) async fn confirm_bulk_delete(cx: &Cx, Form(form): Form<PageForm>) -> Result<ViewHandle> {
    let ids = match session_key(cx).await? {
        Some(key) => selection::get(pool(cx), &key, RESOURCE).await?,
        None => Vec::new(),
    };
    let error = match bulk::prepare_batch(&ids) {
        Ok(_) => None,
        Err(_) if ids.is_empty() => None,
        Err(err) => Some(err.status_and_message().1),
    };
    let items = if error.is_none() {
        bulk_items(cx, &ids).await?
    } else {
        Vec::new()
    };
    let execute_url = href!(super::execute_bulk_delete).resolve(cx);
    view! {
        cx =>
        ui::bulk_confirm_dialog(
            one: "order",
            many: "orders",
            items: &items,
            error: error.as_deref(),
            execute_url: &execute_url,
            page: form.page()
        )
    }
    .single()
    .await
}
