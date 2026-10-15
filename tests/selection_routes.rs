//! Server-side multi-select: checkbox posts, select-all, the toolbar count,
//! and that the selection survives paging but not a fresh visit.

mod common;

use common::{App, customer, order, row_id_near, seed_customer, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

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

async fn select(app: &App, resource: &str, ids: &str, checked: bool, page: i64) -> String {
    let response = app
        .htmx_post(
            &format!("/{resource}/selection"),
            &[
                ("ids", ids.to_string()),
                ("checked", checked.to_string()),
                ("page", page.to_string()),
                ("query_id", String::new()),
            ],
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    response.body
}

async fn page(app: &App, resource: &str, page: i64) -> String {
    app.htmx_post(
        &format!("/{resource}/page"),
        &[("page", page.to_string()), ("query_id", String::new())],
    )
    .await
    .body
}

/// The `<input>` tag whose `aria-label` is `label`.
fn checkbox<'a>(html: &'a str, label: &str) -> &'a str {
    let at = html
        .find(&format!("aria-label=\"{label}\""))
        .unwrap_or_else(|| panic!("checkbox {label:?} not found"));
    let start = html[..at].rfind("<input").unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

fn is_checked(html: &str, label: &str) -> bool {
    checkbox(html, label).contains(" checked=\"\"")
}

fn customer_id(html: &str, email: &str) -> i64 {
    row_id_near(html, "customer-row-", email)
}

#[sqlx::test]
async fn first_tick_starts_a_session_and_updates_the_toolbar(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 3).await;
    let list = app.get("/customers").await.body;
    assert!(!app.has_session(), "viewing a list starts no session");
    assert!(list.contains("No rows selected"));
    assert!(!is_checked(&list, "Select Pat Cust00"));

    let id = customer_id(&list, "c00@example.com");
    let body = select(&app, "customers", &id.to_string(), true, 1).await;
    assert!(app.has_session());
    assert!(body.contains("id=\"customer-toolbar\" hx-swap-oob=\"true\""));
    assert!(body.contains("1 selected"));
    assert!(is_checked(&body, "Select Pat Cust00"));

    // The row checkbox posts without swapping and includes the list state.
    let input = checkbox(&list, "Select Pat Cust00");
    assert!(input.contains("hx-post=\"/customers/selection\""));
    assert!(input.contains("hx-swap=\"none\""));
    assert!(input.contains("hx-include=\"#customer-list-state\""));
}

#[sqlx::test]
async fn selection_survives_paging(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 15).await;
    let first = app.get("/customers").await.body;
    let id = customer_id(&first, "c03@example.com");
    select(&app, "customers", &id.to_string(), true, 1).await;

    let second = page(&app, "customers", 2).await;
    assert!(!is_checked(&second, "Select Pat Cust10"));
    let back = page(&app, "customers", 1).await;
    assert!(
        is_checked(&back, "Select Pat Cust03"),
        "still ticked after Prev/Next"
    );
    assert!(!is_checked(&back, "Select Pat Cust04"));
}

#[sqlx::test]
async fn select_all_ticks_the_page_and_counts_across_pages(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 12).await;
    let first = app.get("/customers").await.body;
    let all = checkbox(&first, "Select all on this page");
    assert!(all.contains("hx-target=\"#customer-list\""));
    assert!(!all.contains(" checked=\"\""));

    // The header box carries this page's ids as one comma-joined string.
    let ids: Vec<String> = (0..10)
        .map(|i| customer_id(&first, &format!("c{i:02}@example.com")).to_string())
        .collect();
    assert!(all.contains(&ids.join(",")), "{all}");

    let body = select(&app, "customers", &ids.join(","), true, 1).await;
    assert!(body.contains("10 selected"));
    assert!(is_checked(&body, "Select all on this page"));
    assert!((0..10).all(|i| is_checked(&body, &format!("Select Pat Cust{i:02}"))));

    // Page 2 isn't covered by page 1's select-all, but the count is global.
    let second = page(&app, "customers", 2).await;
    assert!(!is_checked(&second, "Select all on this page"));
    let id = customer_id(&second, "c11@example.com");
    let body = select(&app, "customers", &id.to_string(), true, 2).await;
    assert!(body.contains("11 selected"));
    assert!(
        body.contains("Page 2 of 2"),
        "re-renders the page the box is on"
    );

    // Unticking select-all clears just that page.
    let body = select(&app, "customers", &ids.join(","), false, 1).await;
    assert!(body.contains("1 selected"));
    assert!(!is_checked(&body, "Select Pat Cust00"));
}

#[sqlx::test]
async fn selections_are_per_browser(pool: PgPool) {
    let alice = App::new(pool.clone());
    let bob = App::new(pool);
    seed_customers(&alice, 3).await;
    let list = alice.get("/customers").await.body;
    let id = customer_id(&list, "c01@example.com");
    select(&alice, "customers", &id.to_string(), true, 1).await;

    let bobs = bob.get("/customers").await.body;
    assert!(!is_checked(&bobs, "Select Pat Cust01"));
    let body = select(&bob, "customers", "", true, 1).await;
    assert!(
        body.contains("No rows selected"),
        "Bob's own selection is empty"
    );

    let alices = page(&alice, "customers", 1).await;
    assert!(is_checked(&alices, "Select Pat Cust01"));
}

#[sqlx::test]
async fn a_fresh_visit_clears_the_selection(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 3).await;
    let list = app.get("/customers").await.body;
    let id = customer_id(&list, "c00@example.com");
    select(&app, "customers", &id.to_string(), true, 1).await;

    let again = app.get("/customers").await.body;
    assert!(again.contains("No rows selected"));
    assert!(!is_checked(&again, "Select Pat Cust00"));
    let body = select(&app, "customers", "", true, 1).await;
    assert!(body.contains("No rows selected"));
}

#[sqlx::test]
async fn deleting_a_selected_row_drops_it_from_the_count(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 3).await;
    let list = app.get("/customers").await.body;
    let a = customer_id(&list, "c00@example.com");
    let b = customer_id(&list, "c01@example.com");
    select(&app, "customers", &format!("{a},{b}"), true, 1).await;

    let response = app.htmx_delete(&format!("/customers/{a}?page=1")).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("1 selected"));
    assert!(
        response
            .body
            .contains("id=\"customer-toolbar\" hx-swap-oob=\"true\"")
    );
    assert!(is_checked(&response.body, "Select Pat Cust01"));
}

#[sqlx::test]
async fn editing_a_selected_row_keeps_it_ticked(pool: PgPool) {
    let app = App::new(pool);
    seed_customers(&app, 2).await;
    let list = app.get("/customers").await.body;
    let id = customer_id(&list, "c00@example.com");
    select(&app, "customers", &id.to_string(), true, 1).await;

    let response = app
        .htmx_put(
            &format!("/customers/{id}"),
            &customer("Pat", "Renamed", "c00@example.com", ""),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(is_checked(&response.body, "Select Pat Renamed"));
}

#[sqlx::test]
async fn malformed_ids_are_rejected(pool: PgPool) {
    let app = App::new(pool);
    let response = app
        .htmx_post(
            "/customers/selection",
            &[("ids", "1,abc"), ("checked", "true")],
        )
        .await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("Invalid selection."));
}

#[sqlx::test]
async fn products_and_orders_are_selectable_separately(pool: PgPool) {
    let app = App::new(pool);
    let customer_id = seed_customer(&app, "ada@example.com").await;
    let product_id = seed_product(&app, "SKU-1", "1.00").await;
    let response = app
        .htmx_post(
            "/orders",
            &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
        )
        .await;
    let order_id = row_id_near(&response.body, "order-row-", "Ada Lovelace");

    let body = select(&app, "products", &product_id.to_string(), true, 1).await;
    assert!(body.contains("id=\"product-toolbar\""));
    assert!(body.contains("1 selected"));

    let body = select(&app, "orders", &order_id.to_string(), true, 1).await;
    assert!(body.contains("id=\"order-toolbar\""));
    assert!(body.contains("1 selected"), "orders count only orders");
    assert!(is_checked(&body, &format!("Select order {order_id}")));
}
