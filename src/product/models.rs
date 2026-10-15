use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub sku: String,
    pub price_cents: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Raw form input as submitted by the browser/htmx, before validation.
/// `price` is a plain string here (e.g. "19.99") - the service layer parses
/// and converts it to integer cents. `Serialize` is only exercised by
/// integration tests, which build form bodies from this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductForm {
    pub name: String,
    pub sku: String,
    pub price: String,
    #[serde(default)]
    pub description: String,
}

/// Validated, owned data ready for persistence. Only the service layer
/// constructs this, after `ProductForm` has passed validation.
#[derive(Debug, Clone)]
pub struct NewProduct {
    pub name: String,
    pub sku: String,
    pub price_cents: i32,
    pub description: Option<String>,
}
