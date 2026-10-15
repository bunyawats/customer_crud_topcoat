//! Full HTTP integration tests for the product routes.

mod common;

use common::{App, order, product, row_id, seed_customer};
use sqlx::PgPool;
use topcoat::router::StatusCode;

#[sqlx::test]
async fn create_then_list_shows_new_product(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/products",
            &product("Widget", "WID-1", "19.99", "A fine widget."),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("Widget"));

    let response = app.get("/products").await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("WID-1"));
    assert!(response.body.contains("$19.99"));
}

#[sqlx::test]
async fn duplicate_sku_is_rejected_with_htmx_alert(pool: PgPool) {
    let app = App::new(pool);

    app.htmx_post("/products", &product("Widget", "DUP-1", "9.99", ""))
        .await;
    let response = app
        .htmx_post("/products", &product("Gadget", "DUP-1", "14.99", ""))
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("already exists"));
}

#[sqlx::test]
async fn invalid_price_is_rejected_without_touching_database(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post("/products", &product("Widget", "WID-2", "free", ""))
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("Price"));
}

#[sqlx::test]
async fn update_then_delete_removes_product(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post("/products", &product("Widget", "WID-3", "9.99", ""))
        .await;
    let id = row_id(&response.body, "product-row-");

    let response = app
        .htmx_put(
            &format!("/products/{id}"),
            &product("Widget Pro", "WID-3", "29.99", "Upgraded."),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("Widget Pro"));
    assert!(response.body.contains("$29.99"));

    let response = app.htmx_delete(&format!("/products/{id}")).await;
    assert_eq!(response.status, StatusCode::OK);

    let response = app.htmx_get(&format!("/products/{id}")).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn new_product_form_renders_empty_fields(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/products/new").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"create-product-form\""));
    assert!(response.body.contains("hx-post=\"/products\""));
}

#[sqlx::test]
async fn edit_form_is_prefilled(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/products",
            &product("Widget", "WID-4", "19.5", "Sturdy & small."),
        )
        .await;
    let id = row_id(&response.body, "product-row-");

    let response = app.htmx_get(&format!("/products/{id}/edit")).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"edit-product-form\""));
    assert!(response.body.contains("value=\"19.50\""));
    // Text content is escaped by `view!`.
    assert!(response.body.contains("Sturdy &amp; small."));
}

#[sqlx::test]
async fn editing_unknown_product_returns_not_found(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/products/999999/edit").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body.contains("not found"));
}

#[sqlx::test]
async fn deleting_product_used_in_order_is_rejected(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post("/products", &product("Widget", "WID-USED-1", "9.99", ""))
        .await;
    let product_id = row_id(&response.body, "product-row-");
    let customer_id = seed_customer(&app, "ada-prod-test@example.com").await;

    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "pending", &[(product_id, 2)]),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);

    let response = app.htmx_delete(&format!("/products/{product_id}")).await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("Cannot delete this product"));
    assert!(response.body.contains("order"));

    // The product must still exist - the delete was rejected, not silently
    // ignored.
    let response = app.htmx_get(&format!("/products/{product_id}")).await;
    assert_eq!(response.status, StatusCode::OK);
}
