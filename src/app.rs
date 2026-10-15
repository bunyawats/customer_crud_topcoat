//! The web layer, routed by module: this module is `/`, and every child
//! module adds one URL segment (`customers` → `/customers`,
//! `customers::customer_id` → `/customers/{customer_id}`).
//!
//! Besides the router itself, this module holds the three things that apply
//! to every request: the document layout, the error layer, and the branded
//! 404 for unrouted URLs.

pub(crate) mod customers;
pub(crate) mod orders;
pub(crate) mod products;

use std::collections::HashSet;

use serde::Deserialize;
use sqlx::PgPool;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, app_context},
    cookie::RouterBuilderCookieExt,
    htmx::{HxReswap, SwapOption, hx_request},
    router::{
        Body, Next, Router, Slot, StatusCode,
        error::{BadRequestError, NotFoundError, SeeOther, see_other},
        href, layer, layout, module_router, not_found, query_params,
        response::{IntoResponse, Response},
        route,
    },
    session::{self, RouterBuilderSessionExt, SessionConfig},
    view::{View, ViewExt, view},
};

use crate::{
    error::AppError,
    pagination::PageRequest,
    selection::{models::Resource, services as selection},
    ui,
};

/// Builds the application router. Split out from `main` so integration
/// tests can dispatch real HTTP requests through it with `Router::handle`.
pub fn router(pool: PgPool, assets: AssetBundle) -> Router {
    module_router!()
        .app_context(pool)
        .assets(assets)
        .cookies()
        .sessions(SessionConfig::default())
        .build()
}

/// The shared connection pool, registered as app context in [`router`].
pub(crate) fn pool(cx: &Cx) -> &PgPool {
    app_context::<PgPool>(cx)
}

/// The pager's Prev/Next fields (`POST /<resource>/page`).
#[derive(Deserialize)]
pub(crate) struct PageForm {
    #[serde(default = "first_page")]
    page: i64,
    #[serde(default)]
    query_id: String,
}

fn first_page() -> i64 {
    1
}

impl PageForm {
    pub(crate) fn request(&self) -> PageRequest {
        PageRequest::new(self.page, &self.query_id)
    }

    pub(crate) fn page(&self) -> i64 {
        self.page
    }

    /// The same page with a fresh count, for re-rendering after a write.
    pub(crate) fn after_write(&self) -> PageRequest {
        PageRequest::new(self.page, "")
    }
}

/// A checkbox post (`POST /<resource>/selection`): `ids` is one id or a
/// comma-joined page of them; `page`/`query_id` come from the list's
/// `hx-include`d state so the response can re-render the same page.
#[derive(Deserialize)]
pub(crate) struct SelectionForm {
    #[serde(default)]
    ids: String,
    #[serde(default)]
    checked: bool,
    #[serde(default = "first_page")]
    page: i64,
    #[serde(default)]
    query_id: String,
}

impl SelectionForm {
    pub(crate) fn page_request(&self) -> PageRequest {
        PageRequest::new(self.page, &self.query_id)
    }
}

/// Applies a checkbox post to this browser's selection (starting a session
/// on the first tick) and returns the selection afterwards.
pub(crate) async fn apply_selection(
    cx: &Cx,
    resource: Resource,
    form: &SelectionForm,
) -> Result<HashSet<i64>> {
    let ids = selection::parse_ids(&form.ids)?;
    let key = ensure_session_key(cx).await?;
    let selected = selection::update(pool(cx), &key, resource, &ids, form.checked).await?;
    Ok(selected.into_iter().collect())
}

/// A fresh visit to a list page starts with nothing selected (the
/// skill's rule: clear on navigation, never on Prev/Next).
pub(crate) async fn clear_selection(cx: &Cx, resource: Resource) -> Result<()> {
    if let Some(key) = session_key(cx).await? {
        selection::clear(pool(cx), &key, resource).await?;
    }
    Ok(())
}

/// The list's current page, from the query string: htmx sends a `DELETE`'s
/// `hx-include`d fields there, not in a body.
#[query_params(error = bad_request)]
pub(crate) struct ListQuery {
    page: Option<i64>,
}

/// The page the user was on, for re-rendering after a write. No `query_id`:
/// the write changed the count, so recompute it.
pub(crate) fn page_after_write(cx: &Cx) -> Result<PageRequest> {
    let query = query_params::<ListQuery>(cx)?;
    Ok(PageRequest::new(query.page.unwrap_or(1), ""))
}

/// This browser's selection key: its session token's hash, or `None` if it
/// has no session yet (nothing selected). Read-only; safe in any handler.
pub(crate) async fn session_key(cx: &Cx) -> Result<Option<[u8; 32]>> {
    Ok(session::token_hash(cx).await?.map(|hash| *hash))
}

/// The selection key, starting an anonymous session first if needed. It
/// sets a cookie, so call it before building the response view.
pub(crate) async fn ensure_session_key(cx: &Cx) -> Result<[u8; 32]> {
    if let Some(key) = session_key(cx).await? {
        return Ok(key);
    }
    Ok(*session::start(cx).await?.token_hash)
}

/// What this browser has selected on `resource`'s list (empty without a
/// session), for rendering checkbox states.
pub(crate) async fn selected_ids(cx: &Cx, resource: Resource) -> Result<HashSet<i64>> {
    let Some(key) = session_key(cx).await? else {
        return Ok(HashSet::new());
    };
    Ok(selection::get(pool(cx), &key, resource)
        .await?
        .into_iter()
        .collect())
}

/// `/` has no page of its own.
#[route(GET)]
async fn home(cx: &Cx) -> Result<SeeOther> {
    Ok(see_other(href!(customers::index).resolve(cx)))
}

/// Wraps every `#[page]` in the document shell. `#[route]` handlers (the
/// htmx fragment endpoints) are not wrapped, so they return bare fragments.
#[layout]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! { ui::shell((slot)) })
}

/// Turns errors into responses for every handler, so handlers can just use
/// `?`. htmx requests get a 4xx/5xx with the out-of-band alert fragment and
/// `HX-Reswap: none` (the element that made the request stays as it was);
/// normal navigations get a full error page. Redirects, rewrites and any
/// other framework error pass through to Topcoat untouched.
#[layer]
async fn handle_errors(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let error = match next.run(cx, body).await {
        Ok(response) => return Ok(response),
        Err(error) => error,
    };

    let (status, message) = if let Some(app_error) = error.downcast_ref::<AppError>() {
        app_error.status_and_message()
    } else if error.is::<NotFoundError>() {
        (StatusCode::NOT_FOUND, "Page not found.".to_string())
    } else if let Some(bad_request) = error.downcast_ref::<BadRequestError>() {
        (
            StatusCode::BAD_REQUEST,
            bad_request.description().to_string(),
        )
    } else {
        return Err(error);
    };

    if hx_request(cx) {
        let fragment = view! {
            cx =>
            (status)
            ui::alert(message: &message)
        }
        .single()
        .await?;
        (HxReswap(SwapOption::None), fragment).into_response(cx)
    } else {
        let title = format!("Error {}", status.as_u16());
        view! {
            cx =>
            (status)
            ui::shell(
                title: Some(title),
                ui::error_page(status: status, message: &message)
            )
        }
        .single()
        .await?
        .into_response(cx)
    }
}

// Unrouted URLs become a `NotFoundError` from a catch-all page, so they go
// through `handle_errors` and get the branded 404 instead of a bare one.
not_found!();
