//! Bulk delete: the confirm dialog, the fan-out over the single-item delete
//! (guards and all), and what the response re-renders.

mod common;

use common::{App, customer, order, row_id_near, seed_customer, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

async fn seed_customers(app: &App, n: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for i in 0..n {
        let email = format!("c{i:02}@example.com");
        let response = app
            .htmx_post(
                "/customers",
                &customer("Pat", &format!("Cust{i:02}"), &email, ""),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
        ids.push(row_id_near(&response.body, "customer-row-", &email));
    }
    ids
}

async fn select(app: &App, resource: &str, ids: &[i64]) {
    let joined = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let response = app
        .htmx_post(
            &format!("/{resource}/selection"),
            &[("ids", joined.as_str()), ("checked", "true"), ("page", "1")],
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
}

async fn confirm(app: &App, resource: &str) -> String {
    let response = app
        .htmx_post(
            &format!("/{resource}/bulk-delete/confirm"),
            &[("page", "1")],
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    response.body
}

async fn execute(app: &App, resource: &str) -> common::TestResponse {
    app.htmx_post(&format!("/{resource}/bulk-delete"), &[("page", "1")])
        .await
}

async fn customer_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM customers")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn confirm_lists_the_selection_with_fresh_names(pool: PgPool) {
    let app = App::new(pool);
    let ids = seed_customers(&app, 3).await;
    select(&app, "customers", &ids[..2]).await;

    let body = confirm(&app, "customers").await;
    assert!(body.contains("Delete 2 customers?"));
    assert!(body.contains("Pat Cust00") && body.contains("c00@example.com"));
    assert!(body.contains("Pat Cust01"));
    assert!(!body.contains("Pat Cust02"));
    assert!(body.contains("hx-post=\"/customers/bulk-delete\""));
    assert!(body.contains(">Delete 2 customers<"), "submit button");
    // The execute route re-reads the selection; the form carries no ids.
    assert!(!body.contains("name=\"ids\""));

    // The toolbar button loads this dialog with the list's page state.
    let list = app.get("/customers").await.body;
    assert!(list.contains("hx-post=\"/customers/bulk-delete/confirm\""));
}

#[sqlx::test]
async fn confirm_with_nothing_selected_offers_no_delete(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 1).await;
    let body = confirm(&app, "customers").await;
    assert!(body.contains("Nothing is selected."));
    assert!(!body.contains("type=\"submit\""));
}

#[sqlx::test]
async fn confirm_drops_rows_deleted_elsewhere(pool: PgPool) {
    let app = App::new(pool.clone());
    let ids = seed_customers(&app, 2).await;
    select(&app, "customers", &ids).await;
    sqlx::query("DELETE FROM customers WHERE id = $1")
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();

    let body = confirm(&app, "customers").await;
    assert!(body.contains("Delete 1 customer?"));
    assert!(!body.contains("Pat Cust00"));
}

#[sqlx::test]
async fn execute_deletes_clears_the_selection_and_refreshes(pool: PgPool) {
    let app = App::new(pool.clone());
    let ids = seed_customers(&app, 3).await;
    select(&app, "customers", &ids[..2]).await;

    let response = execute(&app, "customers").await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let body = response.body;
    assert!(body.contains("Deleted 2 customers"));
    assert!(body.contains("Every selected row was deleted."));
    assert!(body.contains("id=\"customer-list\" hx-swap-oob=\"true\""));
    assert!(body.contains("id=\"customer-toolbar\" hx-swap-oob=\"true\""));
    assert!(body.contains("No rows selected"));
    assert!(body.contains("Showing 1–1 of 1"));
    assert!(body.contains("Pat Cust02"));
    assert_eq!(customer_count(&pool).await, 1);

    assert!(
        confirm(&app, "customers")
            .await
            .contains("Nothing is selected.")
    );
}

#[sqlx::test]
async fn guarded_rows_are_reported_and_the_rest_deleted(pool: PgPool) {
    let app = App::new(pool.clone());
    let free = seed_customers(&app, 1).await[0];
    let guarded = seed_customer(&app, "ada@example.com").await;
    let product_id = seed_product(&app, "SKU-1", "1.00").await;
    app.htmx_post(
        "/orders",
        &order(&guarded.to_string(), "pending", &[(product_id, 1)]),
    )
    .await;
    select(&app, "customers", &[free, guarded]).await;

    let response = execute(&app, "customers").await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let body = response.body;
    assert!(body.contains("Deleted 1 customer"));
    assert!(body.contains("1 customer could not be deleted:"));
    assert!(body.contains("Ada Lovelace"), "failure labelled by name");
    assert!(body.contains("Cannot delete this customer: 1 order still references them."));
    assert_eq!(customer_count(&pool).await, 1, "the guarded customer stays");
    // Cleared whatever the outcome.
    assert!(body.contains("No rows selected"));

    // Same for products in use.
    select(&app, "products", &[product_id]).await;
    let body = execute(&app, "products").await.body;
    assert!(body.contains("Deleted 0 products"));
    assert!(body.contains("Cannot delete this product: it is used in 1 order."));
}

#[sqlx::test]
async fn over_the_batch_cap_nothing_is_deleted(pool: PgPool) {
    let app = App::new(pool.clone());
    sqlx::query(
        "INSERT INTO customers (first_name, last_name, email)
         SELECT 'Bulk', 'Row', 'bulk' || n || '@example.com' FROM generate_series(1, 101) AS n",
    )
    .execute(&pool)
    .await
    .unwrap();
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM customers")
        .fetch_all(&pool)
        .await
        .unwrap();
    select(&app, "customers", &ids).await;

    let body = confirm(&app, "customers").await;
    assert!(body.contains("Select at most 100 rows at a time (101 selected)."));
    assert!(!body.contains("type=\"submit\""));

    let response = execute(&app, "customers").await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert_eq!(customer_count(&pool).await, 101);
}

#[sqlx::test]
async fn execute_with_nothing_selected_is_rejected(pool: PgPool) {
    let app = App::new(pool);
    let response = execute(&app, "customers").await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("Nothing is selected."));
}

#[sqlx::test]
async fn one_browsers_bulk_delete_only_touches_its_own_selection(pool: PgPool) {
    let alice = App::new(pool.clone());
    let bob = App::new(pool.clone());
    let ids = seed_customers(&alice, 3).await;
    select(&alice, "customers", &ids[..1]).await;
    select(&bob, "customers", &ids[..2]).await;

    let body = execute(&alice, "customers").await.body;
    assert!(body.contains("Deleted 1 customer"));
    assert_eq!(customer_count(&pool).await, 2);

    // Bob's selection lost the row Alice deleted, and kept the other.
    let bobs = confirm(&bob, "customers").await;
    assert!(bobs.contains("Delete 1 customer?"));
    assert!(bobs.contains("Pat Cust01"));
}

#[sqlx::test]
async fn orders_bulk_delete(pool: PgPool) {
    let app = App::new(pool);
    let customer_id = seed_customer(&app, "ada@example.com").await;
    let product_id = seed_product(&app, "SKU-1", "1.00").await;
    let mut ids = Vec::new();
    for _ in 0..2 {
        let response = app
            .htmx_post(
                "/orders",
                &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
            )
            .await;
        ids.push(row_id_near(&response.body, "order-row-", "Ada Lovelace"));
    }
    select(&app, "orders", &ids).await;

    let body = confirm(&app, "orders").await;
    assert!(body.contains("Delete 2 orders?"));
    assert!(body.contains(&format!("Order #{}", ids[0])));

    let body = execute(&app, "orders").await.body;
    assert!(body.contains("Deleted 2 orders"));
    assert!(body.contains("id=\"order-list\" hx-swap-oob=\"true\""));
}
