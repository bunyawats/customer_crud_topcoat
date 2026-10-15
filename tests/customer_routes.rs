//! Full HTTP integration tests for the customer routes.

mod common;

use common::{App, customer, order, row_id, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

#[sqlx::test]
async fn create_then_list_shows_new_customer(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/customers",
            &customer("Ada", "Lovelace", "ada@example.com", "555-1234"),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("Ada Lovelace"));

    let response = app.get("/customers").await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("ada@example.com"));
    // A normal navigation gets the whole document from the layout.
    assert!(response.body.starts_with("<!DOCTYPE html>"));
    assert!(response.body.contains("<title>Customer Profiles</title>"));
}

#[sqlx::test]
async fn duplicate_email_is_rejected_with_htmx_alert(pool: PgPool) {
    let app = App::new(pool);

    app.htmx_post(
        "/customers",
        &customer("Ada", "Lovelace", "dup@example.com", ""),
    )
    .await;
    let response = app
        .htmx_post(
            "/customers",
            &customer("Grace", "Hopper", "dup@example.com", ""),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("already exists"));
    assert!(response.body.contains("hx-swap-oob=\"true\""));
}

#[sqlx::test]
async fn invalid_email_is_rejected_without_touching_database(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/customers",
            &customer("Ada", "Lovelace", "not-an-email", ""),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("valid email"));
}

#[sqlx::test]
async fn update_then_delete_removes_customer(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/customers",
            &customer("Ada", "Lovelace", "ada2@example.com", ""),
        )
        .await;
    let id = row_id(&response.body, "customer-row-");

    let response = app
        .htmx_put(
            &format!("/customers/{id}"),
            &customer("Ada", "King", "ada2@example.com", ""),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("Ada King"));

    // Delete re-renders the list region (so the pager's counts stay right),
    // without the deleted row.
    let response = app.htmx_delete(&format!("/customers/{id}")).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"customer-list\""));
    assert!(!response.body.contains(&format!("customer-row-{id}\"")));

    let response = app.htmx_get(&format!("/customers/{id}")).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn new_customer_form_renders_empty_fields(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/customers/new").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"create-customer-form\""));
    assert!(response.body.contains("hx-post=\"/customers\""));
    // A fragment endpoint, not wrapped in the layout.
    assert!(!response.body.contains("<html"));
}

#[sqlx::test]
async fn edit_form_is_prefilled(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/customers",
            &customer("Ada", "Lovelace", "ada5@example.com", "555-0000"),
        )
        .await;
    let id = row_id(&response.body, "customer-row-");

    let response = app.htmx_get(&format!("/customers/{id}/edit")).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"edit-customer-form\""));
    assert!(
        response
            .body
            .contains(&format!("hx-put=\"/customers/{id}\""))
    );
    assert!(response.body.contains("value=\"ada5@example.com\""));
    assert!(response.body.contains("value=\"555-0000\""));
}

#[sqlx::test]
async fn editing_unknown_customer_returns_not_found(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/customers/999999/edit").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body.contains("not found"));
}

#[sqlx::test]
async fn non_numeric_id_is_a_bad_request(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/customers/abc").await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("Invalid customer id."));
}

#[sqlx::test]
async fn deleting_customer_with_orders_is_rejected(pool: PgPool) {
    let app = App::new(pool);

    let response = app
        .htmx_post(
            "/customers",
            &customer("Ada", "Lovelace", "ada3@example.com", ""),
        )
        .await;
    let customer_id = row_id(&response.body, "customer-row-");
    let product_id = seed_product(&app, "WID-CUST-1", "9.99").await;

    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);

    let response = app.htmx_delete(&format!("/customers/{customer_id}")).await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("Cannot delete this customer"));
    assert!(response.body.contains("order"));

    // The customer must still exist - the delete was rejected, not silently
    // ignored.
    let response = app.htmx_get(&format!("/customers/{customer_id}")).await;
    assert_eq!(response.status, StatusCode::OK);
}
