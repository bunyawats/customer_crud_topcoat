//! App-wide behavior: the root redirect, the error layer's two response
//! shapes, and the branded 404.

mod common;

use common::App;
use sqlx::PgPool;
use topcoat::router::StatusCode;

#[sqlx::test]
async fn root_redirects_to_customers(pool: PgPool) {
    let app = App::new(pool);

    let response = app.get("/").await;

    assert_eq!(response.status, StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), Some("/customers"));
}

#[sqlx::test]
async fn errors_on_normal_navigation_render_a_full_page(pool: PgPool) {
    let app = App::new(pool);

    let response = app.get("/customers/999999/edit").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.header("hx-reswap"), None);
    assert!(response.body.starts_with("<!DOCTYPE html>"));
    assert!(response.body.contains("<title>Error 404</title>"));
    assert!(response.body.contains("Customer not found."));
}

#[sqlx::test]
async fn unrouted_urls_get_the_branded_404(pool: PgPool) {
    let app = App::new(pool);

    let response = app.get("/no/such/page").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body.contains("Page not found."));
    assert!(response.body.contains("id=\"crud-dialog\""));
}

#[sqlx::test]
async fn nav_highlights_the_current_section(pool: PgPool) {
    let app = App::new(pool);

    let response = app.get("/products").await;

    assert!(response.body.contains(
        "<a href=\"/products\" class=\"nav-link nav-link-active\" aria-current=\"page\">"
    ));
    assert!(
        response
            .body
            .contains("<a href=\"/customers\" class=\"nav-link\">")
    );
}
