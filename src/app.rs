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

use sqlx::PgPool;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, app_context},
    htmx::{HxReswap, SwapOption, hx_request},
    router::{
        Body, Next, Router, Slot, StatusCode,
        error::{BadRequestError, NotFoundError, SeeOther, see_other},
        href, layer, layout, module_router, not_found,
        response::{IntoResponse, Response},
        route,
    },
    view::{View, ViewExt, view},
};

use crate::{error::AppError, ui};

/// Builds the application router. Split out from `main` so integration
/// tests can dispatch real HTTP requests through it with `Router::handle`.
pub fn router(pool: PgPool, assets: AssetBundle) -> Router {
    module_router!().app_context(pool).assets(assets).build()
}

/// The shared connection pool, registered as app context in [`router`].
pub(crate) fn pool(cx: &Cx) -> &PgPool {
    app_context::<PgPool>(cx)
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
