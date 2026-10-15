//! Paged lists: Prev/Next, the `query_id` count cache, clamping, and how
//! create/delete re-render the list region.

mod common;

use common::{App, customer, order, product, row_id_near, seed_customer, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

/// Seeds `n` customers through the create route, sorted `Cust00`, `Cust01`...
async fn seed_customers(app: &App, n: usize) {
    for i in 0..n {
        let response = app
            .htmx_post(
                "/customers",
                &customer(
                    "Pat",
                    &format!("Cust{i:02}"),
                    &format!("c{i:02}@example.com"),
                    "",
                ),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }
}

async fn page(app: &App, resource: &str, page: i64, query_id: &str) -> String {
    let response = app
        .htmx_post(
            &format!("/{resource}/page"),
            &[
                ("page", page.to_string()),
                ("query_id", query_id.to_string()),
            ],
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    response.body
}

/// The `query_id` the list region was rendered with (its hidden input).
fn query_id(html: &str) -> String {
    let marker = "name=\"query_id\" value=\"";
    let start = html.find(marker).expect("query_id input") + marker.len();
    let end = html[start..].find('"').unwrap();
    html[start..start + end].to_string()
}

/// Whether the pager button labelled `label` ("Prev"/"Next") is disabled.
fn button_disabled(html: &str, label: &str) -> bool {
    let label_pos = html.find(&format!(">{label}<")).expect("pager button");
    let tag_start = html[..label_pos].rfind("<button").unwrap();
    // `disabled=""`, not a bare " disabled": the class list has `disabled:` variants.
    html[tag_start..label_pos].contains(" disabled=\"\"")
}

#[sqlx::test]
async fn first_page_then_next_shows_the_following_ten(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 25).await;

    let first = app.get("/customers").await.body;
    assert!(first.contains("Showing 1–10 of 25"), "{first}");
    assert!(first.contains("Cust00") && first.contains("Cust09"));
    assert!(!first.contains("Cust10"));
    assert!(button_disabled(&first, "Prev"));
    assert!(!button_disabled(&first, "Next"));

    let second = page(&app, "customers", 2, &query_id(&first)).await;
    assert!(second.contains("id=\"customer-list\""));
    assert!(!second.contains("<html"), "a fragment, not a page");
    assert!(second.contains("Showing 11–20 of 25"));
    assert!(second.contains("Cust10") && second.contains("Cust19"));
    assert!(!second.contains("Cust09") && !second.contains("Cust20"));
    assert!(!button_disabled(&second, "Prev") && !button_disabled(&second, "Next"));

    let last = page(&app, "customers", 3, &query_id(&second)).await;
    assert!(last.contains("Showing 21–25 of 25"));
    assert!(last.contains("Page 3 of 3"));
    assert!(button_disabled(&last, "Next"));
}

#[sqlx::test]
async fn page_past_the_end_clamps_to_the_last_page(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 25).await;

    let body = page(&app, "customers", 9, "").await;
    assert!(body.contains("Page 3 of 3"));
    assert!(body.contains("Showing 21–25 of 25"));

    let body = page(&app, "customers", -1, "").await;
    assert!(body.contains("Page 1 of 3"));
}

#[sqlx::test]
async fn query_id_reuses_the_cached_count(pool: PgPool) {
    let app = App::new(pool.clone());
    seed_customers(&app, 12).await;
    let first = app.get("/customers").await.body;
    let cached = query_id(&first);

    // A row added behind the app's back: the cached count doesn't see it,
    // a fresh request (no query_id) does.
    sqlx::query(
        "INSERT INTO customers (first_name, last_name, email) VALUES ('Zed', 'Zulu', 'z@example.com')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let reused = page(&app, "customers", 2, &cached).await;
    assert!(reused.contains("of 12"), "cached total reused");
    assert_eq!(query_id(&reused), cached);

    let fresh = page(&app, "customers", 2, "").await;
    assert!(fresh.contains("of 13"), "no query_id recounts");
    assert_ne!(query_id(&fresh), cached);

    let unknown = page(&app, "customers", 2, "q_000000000000").await;
    assert!(unknown.contains("of 13"), "unknown query_id recounts");
}

#[sqlx::test]
async fn create_shows_the_page_holding_the_new_row(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 25).await;

    // Sorts last, so it lands on page 3.
    let response = app
        .htmx_post(
            "/customers",
            &customer("Zed", "Zulu", "zed@example.com", ""),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"customer-list\""));
    assert!(response.body.contains("zed@example.com"));
    assert!(response.body.contains("Page 3 of 3"));
    assert!(response.body.contains("Showing 21–26 of 26"));

    // The create form swaps the whole list region.
    let form = app.htmx_get("/customers/new").await.body;
    assert!(form.contains("hx-target=\"#customer-list\""));
    assert!(form.contains("hx-swap=\"outerHTML\""));
}

#[sqlx::test]
async fn delete_re_renders_the_page_the_user_was_on(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 21).await;

    // Page 3 holds only Cust20; deleting it falls back to page 2.
    let last = page(&app, "customers", 3, "").await;
    let id = row_id_near(&last, "customer-row-", "c20@example.com");
    let response = app.htmx_delete(&format!("/customers/{id}?page=3")).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("Page 2 of 2"));
    assert!(response.body.contains("Showing 11–20 of 20"));

    // Deleting from page 1 refills it from page 2.
    let first = app.get("/customers").await.body;
    let id = row_id_near(&first, "customer-row-", "c00@example.com");
    let response = app.htmx_delete(&format!("/customers/{id}?page=1")).await;
    assert!(response.body.contains("Showing 1–10 of 19"));
    assert!(
        response.body.contains("Cust10"),
        "first row of old page 2 moved up"
    );

    // Row delete buttons swap the list and include its page state.
    assert!(response.body.contains("hx-target=\"#customer-list\""));
    assert!(
        response
            .body
            .contains("hx-include=\"#customer-list-state\"")
    );
}

#[sqlx::test]
async fn tied_names_page_without_duplicates_or_gaps(pool: PgPool) {
    let app = App::new(pool);
    for i in 0..12 {
        app.htmx_post(
            "/customers",
            &customer("Same", "Name", &format!("same{i:02}@example.com"), ""),
        )
        .await;
    }

    let first = app.get("/customers").await.body;
    let second = page(&app, "customers", 2, &query_id(&first)).await;
    for i in 0..12 {
        let email = format!("same{i:02}@example.com");
        let on_first = first.contains(&email);
        let on_second = second.contains(&email);
        assert!(on_first != on_second, "{email} on exactly one page");
    }
}

#[sqlx::test]
async fn products_and_orders_page_too(pool: PgPool) {
    let app = App::new(pool);
    let customer_id = seed_customer(&app, "ada@example.com").await;
    let mut product_id = 0;
    for i in 0..11 {
        product_id = seed_product(&app, &format!("SKU-{i:02}"), "1.00").await;
    }
    for _ in 0..11 {
        let response = app
            .htmx_post(
                "/orders",
                &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }

    let products = page(&app, "products", 2, "").await;
    assert!(products.contains("id=\"product-list\""));
    assert!(products.contains("Showing 11–11 of 11"));

    let orders = page(&app, "orders", 2, "").await;
    assert!(orders.contains("id=\"order-list\""));
    assert!(orders.contains("Showing 11–11 of 11"));
    // Newest first, so page 2 holds the very first order.
    assert!(orders.contains("order-row-1\""));
}

#[sqlx::test]
async fn product_form_still_creates_into_the_list(pool: PgPool) {
    let app = App::new(pool);
    let response = app
        .htmx_post("/products", &product("Widget", "W-1", "2.50", ""))
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.body.contains("id=\"product-list\""));
    assert!(response.body.contains("W-1"));
}

/// Column widths come from the header, not each page's rows, so they stay put
/// across Prev/Next: every list renders a fixed-layout table with sized `<th>`s,
/// on the full page and on the paged fragment alike.
#[sqlx::test]
async fn lists_use_fixed_column_widths(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 11).await;

    for (resource, sized_header) in [
        ("customers", "w-40\">Phone</th>"),
        ("products", "w-32\">SKU</th>"),
        ("orders", "w-24\">Order</th>"),
    ] {
        let body = app.get(&format!("/{resource}")).await.body;
        assert!(body.contains("table-fixed"), "{resource}: {body}");
        assert!(body.contains(sized_header), "{resource}: {body}");
    }

    let body = page(&app, "customers", 2, "").await;
    assert!(body.contains("table-fixed"), "{body}");
    assert!(body.contains("w-40\">Phone</th>"), "{body}");
}
