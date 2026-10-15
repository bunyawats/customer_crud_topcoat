use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Customer {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Raw form input as submitted by the browser/htmx, before validation.
/// `Serialize` is only exercised by integration tests, which build form
/// bodies from this struct to drive real HTTP requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerForm {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    #[serde(default)]
    pub phone: String,
}

/// Validated, owned data ready for persistence. Only the service layer
/// constructs this, after `CustomerForm` has passed validation.
#[derive(Debug, Clone)]
pub struct NewCustomer {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
}
