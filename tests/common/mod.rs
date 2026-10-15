//! Shared harness for the HTTP integration tests.
//!
//! Requests go through the real Topcoat `Router` (`Router::handle`), with the
//! real layout, error layer and module routes - no server, no sockets.
//!
//! Each test gets its own database from `#[sqlx::test]` (a fresh, migrated,
//! throwaway Postgres database, dropped afterwards), so runs never leave
//! data in the development database.

#![allow(dead_code)] // each test binary uses a different subset

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use serde::Serialize;
use sqlx::PgPool;
use topcoat::{
    asset::AssetBundle,
    router::{
        Body, HeaderMap, Method, Router, StatusCode, header, request::Request, response::Response,
        to_bytes,
    },
};

use customer_crud_topcoat::{
    customer::models::CustomerForm,
    order::models::{LineItemInput, OrderForm},
    product::models::ProductForm,
};

/// Pages render `asset!` URLs (the Tailwind stylesheet, app.js), and
/// rendering an asset missing from the loaded bundle panics. So bundle this
/// test binary's own asset declarations once - all local files, no network -
/// into a directory next to it, exactly as `topcoat asset bundle` would.
fn assets() -> AssetBundle {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let exe = std::env::current_exe().expect("test binary path");
        let binary = std::fs::read(&exe).expect("read test binary");
        let name = exe.file_stem().expect("test binary name").to_string_lossy();
        let dir = exe.with_file_name(format!("{name}-assets"));
        topcoat_asset::Bundler::new(&topcoat_asset::BundlerConfig::new())
            .bundle(&binary, &dir)
            .expect("bundle test assets");
        dir
    });
    AssetBundle::load_dir(dir).expect("load test asset bundle")
}

/// A test client: the real router plus a cookie jar, so a sequence of
/// requests shares one session the way a browser tab does. Two `App`s on the
/// same pool are two separate browsers.
pub struct App {
    router: Router,
    cookies: Mutex<BTreeMap<String, String>>,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl App {
    pub fn new(pool: PgPool) -> Self {
        App {
            router: customer_crud_topcoat::router(pool, assets()),
            cookies: Mutex::new(BTreeMap::new()),
        }
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        htmx: bool,
        form: Option<String>,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(uri);
        if htmx {
            request = request.header("HX-Request", "true");
        }
        if let Some(cookie) = self.cookie_header() {
            request = request.header(header::COOKIE, cookie);
        }
        let body = match form {
            Some(form) => {
                request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
                Body::from(form)
            }
            None => Body::empty(),
        };
        let response: Response = self.router.handle(request.body(body).unwrap()).await;
        let (parts, body) = response.into_parts();
        self.store_cookies(&parts.headers);
        let bytes = to_bytes(body, usize::MAX).await.expect("read body");
        TestResponse {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8(bytes.to_vec()).expect("UTF-8 body"),
        }
    }

    /// A normal browser navigation.
    pub async fn get(&self, uri: &str) -> TestResponse {
        self.send(Method::GET, uri, false, None).await
    }

    pub async fn htmx_get(&self, uri: &str) -> TestResponse {
        self.send(Method::GET, uri, true, None).await
    }

    pub async fn htmx_post(&self, uri: &str, form: &impl Serialize) -> TestResponse {
        self.send(Method::POST, uri, true, Some(encode(form))).await
    }

    pub async fn htmx_put(&self, uri: &str, form: &impl Serialize) -> TestResponse {
        self.send(Method::PUT, uri, true, Some(encode(form))).await
    }

    pub async fn htmx_delete(&self, uri: &str) -> TestResponse {
        self.send(Method::DELETE, uri, true, None).await
    }
}

impl App {
    fn cookie_header(&self) -> Option<String> {
        let jar = self.cookies.lock().unwrap();
        (!jar.is_empty()).then(|| {
            jar.iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    /// Keeps `name=value` from each `Set-Cookie`; `Max-Age=0` deletes.
    fn store_cookies(&self, headers: &HeaderMap) {
        let mut jar = self.cookies.lock().unwrap();
        for value in headers.get_all(header::SET_COOKIE) {
            let value = value.to_str().expect("ASCII Set-Cookie");
            let pair = value.split(';').next().unwrap_or_default();
            let Some((name, val)) = pair.split_once('=') else {
                continue;
            };
            if value.contains("Max-Age=0") {
                jar.remove(name.trim());
            } else {
                jar.insert(name.trim().to_string(), val.trim().to_string());
            }
        }
    }

    /// Whether this client currently holds a session cookie.
    pub fn has_session(&self) -> bool {
        self.cookies.lock().unwrap().contains_key("__Host-session")
    }
}

fn encode(form: &impl Serialize) -> String {
    serde_urlencoded::to_string(form).expect("form should serialize")
}

impl TestResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

/// Extracts the numeric id from the first `<prefix><id>` element id in a
/// rendered fragment, e.g. `row_id(html, "customer-row-")`.
pub fn row_id(html: &str, prefix: &str) -> i64 {
    let start = html.find(prefix).expect("row id marker not found") + prefix.len();
    let rest = &html[start..];
    let end = rest.find(['"', ' ']).expect("closing delimiter not found");
    rest[..end].parse().expect("row id should be numeric")
}

/// Like [`row_id`], but for the row containing `anchor` (unique text such as
/// a SKU or email). Create responses re-render every row in sort order, and
/// sort ties between same-named rows mean "the first marker in the document"
/// isn't reliably the row that was just created.
pub fn row_id_near(html: &str, prefix: &str, anchor: &str) -> i64 {
    let anchor_pos = html
        .find(anchor)
        .unwrap_or_else(|| panic!("anchor {anchor:?} not found in response"));
    let marker_pos = html[..anchor_pos]
        .rfind(prefix)
        .expect("row id marker not found before anchor");
    row_id(&html[marker_pos..], prefix)
}

// --- Form builders ----------------------------------------------------------

pub fn customer(first: &str, last: &str, email: &str, phone: &str) -> CustomerForm {
    CustomerForm {
        first_name: first.to_string(),
        last_name: last.to_string(),
        email: email.to_string(),
        phone: phone.to_string(),
    }
}

pub fn product(name: &str, sku: &str, price: &str, description: &str) -> ProductForm {
    ProductForm {
        name: name.to_string(),
        sku: sku.to_string(),
        price: price.to_string(),
        description: description.to_string(),
    }
}

pub fn order(customer_id: &str, status: &str, items: &[(i64, i32)]) -> OrderForm {
    let items: Vec<LineItemInput> = items
        .iter()
        .map(|&(product_id, quantity)| LineItemInput {
            product_id,
            quantity,
        })
        .collect();
    OrderForm {
        customer_id: customer_id.to_string(),
        status: status.to_string(),
        items_json: serde_json::to_string(&items).unwrap(),
    }
}

// --- Seeding through the real routes ----------------------------------------

pub async fn seed_customer(app: &App, email: &str) -> i64 {
    let response = app
        .htmx_post("/customers", &customer("Ada", "Lovelace", email, ""))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    row_id_near(&response.body, "customer-row-", email)
}

pub async fn seed_product(app: &App, sku: &str, price: &str) -> i64 {
    let response = app
        .htmx_post("/products", &product("Widget", sku, price, ""))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    row_id_near(&response.body, "product-row-", sku)
}
