//! List search: what each list matches, that the criteria live server-side
//! (posted once to `/search`, then read back by every list request), how
//! search interacts with the count cache, writes and the selection.

mod common;

use common::{App, customer, order, product, row_id_near, seed_product};
use sqlx::PgPool;
use topcoat::router::StatusCode;

/// Runs a search; the response is the list region (plus the box when clearing).
async fn search(app: &App, resource: &str, q: &str) -> String {
    let response = app
        .htmx_post(&format!("/{resource}/search"), &[("q", q)])
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    response.body
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

async fn select(app: &App, resource: &str, ids: &[i64], checked: bool) {
    let joined = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let response = app
        .htmx_post(
            &format!("/{resource}/selection"),
            &[
                ("ids", joined),
                ("checked", checked.to_string()),
                ("page", "1".to_string()),
            ],
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
}

/// The `query_id` the list region was rendered with (its hidden input).
fn query_id(html: &str) -> String {
    let marker = "name=\"query_id\" value=\"";
    let start = html.find(marker).expect("query_id input") + marker.len();
    let end = html[start..].find('"').unwrap();
    html[start..start + end].to_string()
}

/// The `<input>` tag whose `aria-label` is `label`.
fn input_tag<'a>(html: &'a str, label: &str) -> &'a str {
    let at = html
        .find(&format!("aria-label=\"{label}\""))
        .unwrap_or_else(|| panic!("input {label:?} not found"));
    let start = html[..at].rfind("<input").unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

/// The search box's `<input>` tag.
fn search_box<'a>(html: &'a str, resource_one: &str) -> &'a str {
    let at = html
        .find(&format!("id=\"{resource_one}-search-q\""))
        .expect("search box rendered");
    let start = html[..at].rfind("<input").unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

async fn seed(app: &App, first: &str, last: &str, email: &str, phone: &str) -> i64 {
    let response = app
        .htmx_post("/customers", &customer(first, last, email, phone))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    row_id_near(&response.body, "customer-row-", email)
}

/// Ada, Grace and Alan: distinct names, emails and phones.
async fn seed_people(app: &App) -> (i64, i64, i64) {
    let ada = seed(app, "Ada", "Lovelace", "ada@engine.org", "555-0101").await;
    let grace = seed(app, "Grace", "Hopper", "grace@navy.mil", "").await;
    let alan = seed(app, "Alan", "Turing", "alan@bletchley.uk", "555-0199").await;
    (ada, grace, alan)
}

async fn seed_many(app: &App, n: usize) {
    for i in 0..n {
        seed(
            app,
            "Pat",
            &format!("Cust{i:02}"),
            &format!("c{i:02}@example.com"),
            "",
        )
        .await;
    }
}

async fn filter_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM list_filters")
        .fetch_one(pool)
        .await
        .unwrap()
}

// --- Matching -----------------------------------------------------------------

#[sqlx::test]
async fn customers_match_name_email_or_phone_case_insensitively(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;

    let body = search(&app, "customers", "LOVE").await;
    assert!(body.contains("ada@engine.org"));
    assert!(!body.contains("grace@navy.mil") && !body.contains("alan@bletchley.uk"));

    let body = search(&app, "customers", "ada   lovelace").await;
    assert!(
        body.contains("ada@engine.org"),
        "full name, whitespace collapsed"
    );

    let body = search(&app, "customers", "NAVY").await;
    assert!(body.contains("grace@navy.mil") && !body.contains("ada@engine.org"));

    let body = search(&app, "customers", "0199").await;
    assert!(body.contains("alan@bletchley.uk") && !body.contains("ada@engine.org"));
    assert!(body.contains("Showing 1–1 of 1"), "{body}");
}

#[sqlx::test]
async fn products_match_name_sku_or_description(pool: PgPool) {
    let app = App::new(pool);
    for (name, sku, description) in [
        ("Brass Gear", "GEAR-1", "Fine teeth"),
        ("Steel Spring", "SPR-2", "Coiled tight"),
        ("Copper Wire", "WIRE-3", ""),
    ] {
        let response = app
            .htmx_post("/products", &product(name, sku, "1.00", description))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }

    let body = search(&app, "products", "brass").await;
    assert!(body.contains("GEAR-1") && !body.contains("SPR-2"));
    let body = search(&app, "products", "wire-3").await;
    assert!(body.contains("WIRE-3") && !body.contains("GEAR-1"));
    let body = search(&app, "products", "coiled").await;
    assert!(body.contains("SPR-2") && !body.contains("WIRE-3"));
}

#[sqlx::test]
async fn orders_match_number_or_customer_name(pool: PgPool) {
    let app = App::new(pool.clone());
    let (ada, grace, _) = seed_people(&app).await;
    let product_id = seed_product(&app, "WID-1", "2.00").await;
    for customer_id in [ada, grace, grace] {
        let response = app
            .htmx_post(
                "/orders",
                &order(&customer_id.to_string(), "pending", &[(product_id, 1)]),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM orders ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();

    let body = search(&app, "orders", "grace hop").await;
    assert!(body.contains("Showing 1–2 of 2"), "{body}");
    assert!(!body.contains(&format!("order-row-{}\"", ids[0])));

    for term in [format!("#{}", ids[0]), ids[0].to_string()] {
        let body = search(&app, "orders", &term).await;
        assert!(body.contains("Showing 1–1 of 1"), "{term}: {body}");
        assert!(body.contains(&format!("order-row-{}\"", ids[0])));
    }
}

#[sqlx::test]
async fn like_wildcards_match_literally(pool: PgPool) {
    let app = App::new(pool);
    for (name, sku) in [
        ("50% off", "PCT-1"),
        ("500 off", "NUM-1"),
        ("Gear", "A_1"),
        ("Gear", "AB1"),
    ] {
        let response = app
            .htmx_post("/products", &product(name, sku, "1.00", ""))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    }

    let body = search(&app, "products", "50%").await;
    assert!(body.contains("PCT-1") && !body.contains("NUM-1"), "{body}");
    let body = search(&app, "products", "a_1").await;
    assert!(body.contains("A_1") && !body.contains("AB1"), "{body}");
}

#[sqlx::test]
async fn no_match_shows_the_search_empty_state(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;

    let body = search(&app, "customers", "zzz").await;
    assert!(body.contains("No customers match “zzz”."), "{body}");
    assert!(!body.contains("The ledger is empty."));
}

#[sqlx::test]
async fn an_overlong_term_is_rejected_and_the_stored_filter_kept(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    search(&app, "customers", "ada").await;

    let response = app
        .htmx_post("/customers/search", &[("q", "a".repeat(101))])
        .await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header("hx-reswap"), Some("none"));
    assert!(response.body.contains("at most 100 characters"));

    let body = page(&app, "customers", 1, "").await;
    assert!(body.contains("ada@engine.org") && !body.contains("grace@navy.mil"));
}

// --- Criteria live server-side --------------------------------------------------

#[sqlx::test]
async fn prev_next_stay_filtered_with_only_page_and_query_id(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    seed_many(&app, 25).await;

    let first = search(&app, "customers", "cust").await;
    assert!(first.contains("Showing 1–10 of 25"), "{first}");
    let second = page(&app, "customers", 2, &query_id(&first)).await;
    assert!(second.contains("Showing 11–20 of 25"), "{second}");
    assert!(!second.contains("ada@engine.org"));
}

#[sqlx::test]
async fn criteria_posted_anywhere_but_search_are_ignored(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    search(&app, "customers", "ada").await;

    let response = app
        .htmx_post(
            "/customers/page",
            &[("page", "1"), ("query_id", ""), ("q", "grace")],
        )
        .await;
    assert!(response.body.contains("ada@engine.org"));
    assert!(!response.body.contains("grace@navy.mil"));
}

#[sqlx::test]
async fn a_reload_keeps_the_search_but_clears_the_ticks(pool: PgPool) {
    let app = App::new(pool);
    let (ada, _, _) = seed_people(&app).await;
    search(&app, "customers", "ada").await;
    select(&app, "customers", &[ada], true).await;

    let body = app.get("/customers").await.body;
    assert!(body.contains("ada@engine.org") && !body.contains("grace@navy.mil"));
    assert!(search_box(&body, "customer").contains("value=\"ada\""));
    assert!(body.contains("No rows selected"));
    assert!(!input_tag(&body, "Select Ada Lovelace").contains(" checked=\"\""));
}

#[sqlx::test]
async fn clearing_restores_the_full_list_and_empties_the_box(pool: PgPool) {
    let app = App::new(pool.clone());
    seed_people(&app).await;
    search(&app, "customers", "ada").await;
    assert_eq!(filter_rows(&pool).await, 1);

    let body = search(&app, "customers", "").await;
    assert!(body.contains("grace@navy.mil") && body.contains("ada@engine.org"));
    assert!(
        body.contains("id=\"customer-search\" hx-swap-oob=\"true\""),
        "{body}"
    );
    assert!(search_box(&body, "customer").contains("value=\"\""));
    assert_eq!(filter_rows(&pool).await, 0);

    let body = app.get("/customers").await.body;
    assert!(body.contains("grace@navy.mil"));
}

#[sqlx::test]
async fn browsers_and_lists_keep_separate_filters(pool: PgPool) {
    let app = App::new(pool.clone());
    let other_browser = App::new(pool);
    seed_people(&app).await;
    seed_product(&app, "WID-1", "1.00").await;
    search(&app, "customers", "ada").await;

    let body = other_browser.get("/customers").await.body;
    assert!(
        body.contains("grace@navy.mil"),
        "another browser is unfiltered"
    );

    let body = app.get("/products").await.body;
    assert!(body.contains("WID-1"), "another list is unfiltered");
    assert!(search_box(&body, "product").contains("value=\"\""));
}

// --- Count cache ------------------------------------------------------------------

#[sqlx::test]
async fn a_query_id_from_before_the_search_is_not_reused(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    seed_many(&app, 25).await;

    let unfiltered = app.get("/customers").await.body;
    assert!(unfiltered.contains("of 28"));
    search(&app, "customers", "ada").await;

    let body = page(&app, "customers", 1, &query_id(&unfiltered)).await;
    assert!(body.contains("Showing 1–1 of 1"), "{body}");
}

#[sqlx::test]
async fn pages_clamp_within_the_filtered_total(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    seed_many(&app, 25).await;
    search(&app, "customers", "ada").await;

    let body = page(&app, "customers", 3, "").await;
    assert!(body.contains("Showing 1–1 of 1"), "{body}");
}

// --- Writes -----------------------------------------------------------------------

#[sqlx::test]
async fn row_delete_re_renders_the_same_search_and_page(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;
    seed_many(&app, 25).await;
    let first = search(&app, "customers", "cust").await;
    let second = page(&app, "customers", 2, &query_id(&first)).await;
    let id = row_id_near(&second, "customer-row-", "c15@example.com");

    let response = app.htmx_delete(&format!("/customers/{id}?page=2")).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(
        response.body.contains("Showing 11–20 of 24"),
        "{}",
        response.body
    );
    assert!(!response.body.contains("ada@engine.org"));
}

#[sqlx::test]
async fn bulk_delete_includes_selected_rows_the_search_hides(pool: PgPool) {
    let app = App::new(pool.clone());
    let (ada, grace, _) = seed_people(&app).await;
    select(&app, "customers", &[ada, grace], true).await;
    search(&app, "customers", "grace").await;

    let response = app
        .htmx_post("/customers/bulk-delete/confirm", &[("page", "1")])
        .await;
    assert!(response.body.contains("Ada Lovelace") && response.body.contains("Grace Hopper"));
    assert!(
        response
            .body
            .contains("1 of these isn't in the current search."),
        "{}",
        response.body
    );

    let response = app
        .htmx_post("/customers/bulk-delete", &[("page", "1")])
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let left: Vec<i64> = sqlx::query_scalar("SELECT id FROM customers ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!left.contains(&ada) && !left.contains(&grace));
    // The refreshed list is still under the search, which now matches nothing.
    assert!(response.body.contains("No customers match “grace”."));
}

#[sqlx::test]
async fn the_confirm_has_no_hidden_note_without_a_search(pool: PgPool) {
    let app = App::new(pool);
    let (ada, grace, _) = seed_people(&app).await;
    select(&app, "customers", &[ada, grace], true).await;

    let body = app
        .htmx_post("/customers/bulk-delete/confirm", &[("page", "1")])
        .await
        .body;
    assert!(!body.contains("current search"));
}

#[sqlx::test]
async fn create_clears_the_search_and_shows_the_new_row(pool: PgPool) {
    let app = App::new(pool.clone());
    seed_people(&app).await;
    search(&app, "customers", "ada").await;

    let response = app
        .htmx_post(
            "/customers",
            &customer("Zed", "Zimmer", "zed@example.com", ""),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("zed@example.com"));
    assert!(
        response.body.contains("grace@navy.mil"),
        "list is unfiltered"
    );
    assert!(search_box(&response.body, "customer").contains("value=\"\""));
    assert_eq!(filter_rows(&pool).await, 0);
}

// --- Selection --------------------------------------------------------------------

#[sqlx::test]
async fn ticks_survive_searching_and_clearing(pool: PgPool) {
    let app = App::new(pool);
    let (_, grace, _) = seed_people(&app).await;
    select(&app, "customers", &[grace], true).await;

    search(&app, "customers", "ada").await;
    let body = search(&app, "customers", "grace").await;
    assert!(input_tag(&body, "Select Grace Hopper").contains(" checked=\"\""));
    let body = search(&app, "customers", "").await;
    assert!(input_tag(&body, "Select Grace Hopper").contains(" checked=\"\""));
}

#[sqlx::test]
async fn select_all_covers_only_the_filtered_page(pool: PgPool) {
    let app = App::new(pool);
    let (ada, grace, alan) = seed_people(&app).await;

    let body = search(&app, "customers", "555").await;
    let tag = input_tag(&body, "Select all on this page");
    // Only Ada and Alan have a 555 phone; rows sort by last name.
    assert!(
        tag.contains(&format!("&quot;ids&quot;: &quot;{ada},{alan}&quot;")),
        "{tag}"
    );
    assert!(!body.contains(&format!("customer-row-{grace}\"")));
}

#[sqlx::test]
async fn only_running_a_search_starts_a_session(pool: PgPool) {
    let app = App::new(pool);
    seed_people(&app).await;

    app.get("/customers").await;
    search(&app, "customers", "").await;
    assert!(!app.has_session(), "viewing or clearing starts no session");

    search(&app, "customers", "ada").await;
    assert!(app.has_session());
}
