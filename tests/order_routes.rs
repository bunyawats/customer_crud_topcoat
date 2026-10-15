//! Full HTTP integration tests for the order routes.

mod common;

use common::{App, order, row_id, seed_customer, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

#[sqlx::test]
async fn create_order_computes_total_from_line_items(pool: PgPool) {
    let app = App::new(pool);

    let customer_id = seed_customer(&app, "ada@example.com").await;
    let product_a = seed_product(&app, "WID-A", "10.00").await;
    let product_b = seed_product(&app, "WID-B", "5.50").await;

    let response = app
        .htmx_post(
            "/orders",
            &order(
                &customer_id.to_string(),
                "pending",
                &[(product_a, 2), (product_b, 1)],
            ),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    // 2 * $10.00 + 1 * $5.50 = $25.50
    assert!(response.body.contains("$25.50"));
    assert!(response.body.contains("Ada Lovelace"));

    let response = app.get("/orders").await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("$25.50"));
}

/// Regression: 6 × $7,000,000 = 4,200,000,000 cents, past `i32::MAX`. The
/// total used to be multiplied as INTEGER in SQL and fail with "integer out of
/// range" *after* the order was saved, so every retry added a duplicate.
#[sqlx::test]
async fn order_total_beyond_32_bit_cents_renders(pool: PgPool) {
    let app = App::new(pool.clone());
    let customer_id = seed_customer(&app, "ada@example.com").await;
    let car = seed_product(&app, "CAR-555", "7000000.00").await;

    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "pending", &[(car, 6)]),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("$42000000.00"));

    let id: i64 = sqlx::query_scalar("SELECT id FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = app.htmx_get(&format!("/orders/{id}")).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("$42000000.00"));

    let response = app.get("/orders").await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("$42000000.00"));
}

#[sqlx::test]
async fn orders_with_identical_timestamps_list_newest_id_first(pool: PgPool) {
    let app = App::new(pool.clone());

    let customer_id = seed_customer(&app, "ada@example.com").await;
    let product_id = seed_product(&app, "WID-A", "10.00").await;
    for _ in 0..3 {
        let response = app
            .htmx_post(
                "/orders",
                &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }
    // Rows inserted in one transaction (e.g. a seed script) share `now()`.
    sqlx::query("UPDATE orders SET created_at = '2026-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM orders ORDER BY id DESC")
        .fetch_all(&pool)
        .await
        .unwrap();

    let body = app.get("/orders").await.body;
    let positions: Vec<usize> = ids
        .iter()
        .map(|id| {
            body.find(&format!("order-row-{id}\""))
                .expect("row rendered")
        })
        .collect();
    assert!(
        positions.is_sorted(),
        "expected order rows {ids:?} (newest id first), got positions {positions:?}"
    );
}

#[sqlx::test]
async fn order_referencing_unknown_customer_is_rejected(pool: PgPool) {
    let app = App::new(pool);

    let product_id = seed_product(&app, "WID-C", "9.99").await;

    let response = app
        .htmx_post("/orders", &order("999999", "pending", &[(product_id, 1)]))
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("customer"));
}

#[sqlx::test]
async fn order_with_no_line_items_is_rejected(pool: PgPool) {
    let app = App::new(pool);

    let customer_id = seed_customer(&app, "ada2@example.com").await;

    let response = app
        .htmx_post("/orders", &order(&customer_id.to_string(), "pending", &[]))
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.body.contains("line item"));
}

#[sqlx::test]
async fn update_then_delete_removes_order(pool: PgPool) {
    let app = App::new(pool);

    let customer_id = seed_customer(&app, "ada3@example.com").await;
    let product_a = seed_product(&app, "WID-D", "12.00").await;
    let product_b = seed_product(&app, "WID-E", "3.00").await;

    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "pending", &[(product_a, 1)]),
        )
        .await;
    let id = row_id(&response.body, "order-row-");

    // Replace the single line item with a different product/quantity and
    // change status - the update should fully replace the item set.
    let response = app
        .htmx_put(
            &format!("/orders/{id}"),
            &order(&customer_id.to_string(), "paid", &[(product_b, 3)]),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("$9.00")); // 3 * $3.00
    assert!(response.body.contains("Paid"));

    let response = app.htmx_delete(&format!("/orders/{id}")).await;
    assert_eq!(response.status, StatusCode::OK);

    let response = app.htmx_get(&format!("/orders/{id}")).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn new_order_form_renders_customer_and_product_options(pool: PgPool) {
    let app = App::new(pool);

    seed_customer(&app, "ada4@example.com").await;
    seed_product(&app, "WID-F", "7.25").await;

    let response = app.htmx_get("/orders/new").await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"create-order-form\""));
    assert!(response.body.contains("Ada Lovelace"));
    assert!(response.body.contains("Widget"));
    assert!(response.body.contains("value=\"[]\""));
}

#[sqlx::test]
async fn edit_form_preselects_customer_status_and_items(pool: PgPool) {
    let app = App::new(pool);

    let customer_id = seed_customer(&app, "ada6@example.com").await;
    let product_id = seed_product(&app, "WID-G", "4.00").await;
    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "shipped", &[(product_id, 5)]),
        )
        .await;
    let id = row_id(&response.body, "order-row-");

    let response = app.htmx_get(&format!("/orders/{id}/edit")).await;
    assert_eq!(response.status, StatusCode::OK);
    let body = &response.body;
    assert!(body.contains("id=\"edit-order-form\""));
    assert!(body.contains(&format!("<option value=\"{customer_id}\" selected=\"\">")));
    assert!(body.contains("<option value=\"shipped\" selected=\"\">"));
    assert!(body.contains(&format!("<option value=\"{product_id}\" selected=\"\">")));
    assert!(body.contains("value=\"5\""));
    // The hidden field starts in sync with the rendered rows.
    let items_json = format!("[{{&quot;product_id&quot;:{product_id},&quot;quantity&quot;:5}}]");
    assert!(body.contains(&items_json), "{body}");
}

#[sqlx::test]
async fn editing_unknown_order_returns_not_found(pool: PgPool) {
    let app = App::new(pool);

    let response = app.htmx_get("/orders/999999/edit").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(response.body.contains("not found"));
}
