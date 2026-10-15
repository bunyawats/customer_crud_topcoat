//! `POST /customers/bulk-delete/confirm`: the dialog that asks before a bulk
//! delete.

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, href, route},
    view::{ViewExt, ViewHandle, view},
};

use super::super::{RESOURCE, bulk_items};
use crate::{
    app::{PageForm, list_filter, pool, session_key},
    bulk,
    customer::services as service,
    selection::services as selection,
    ui,
};

/// Lists the selected customers with fresh names (stale ids dropped), or
/// explains why nothing can be deleted (empty, or over the batch cap), and
/// says how many of them the current search hides.
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
    let shown: Vec<i64> = items.iter().map(|item| item.id).collect();
    let filter = list_filter(cx, RESOURCE).await?;
    let hidden = service::count_outside(pool(cx), &shown, &filter).await?;
    let execute_url = href!(super::execute_bulk_delete).resolve(cx);
    view! {
        cx =>
        ui::bulk_confirm_dialog(
            one: "customer",
            many: "customers",
            items: &items,
            error: error.as_deref(),
            execute_url: &execute_url,
            page: form.page(),
            hidden: hidden
        )
    }
    .single()
    .await
}
