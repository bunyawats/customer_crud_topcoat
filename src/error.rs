use std::fmt;

use topcoat::router::StatusCode;

/// Centralized domain error type. Services and repositories never leak
/// `sqlx::Error` or return web-framework responses directly - errors are
/// caught and converted into this enum.
///
/// Route handlers bubble it up with `?` (it converts into `topcoat::Error`
/// like any `std::error::Error`), and the root `#[layer]` in `src/app.rs`
/// downcasts it back out and renders the response: an htmx alert fragment
/// or a full error page. Handlers never build error responses themselves.
#[derive(Debug)]
pub enum AppError {
    Validation(String),
    /// The name of the resource that wasn't found (e.g. "Customer"), used to
    /// build the "<resource> not found." message.
    NotFound(&'static str),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &err
            && let Some(message) = unique_violation_message(db_err.constraint())
        {
            return AppError::Validation(message.to_string());
        }
        AppError::Database(err)
    }
}

/// Friendly messages for unique-constraint violations, keyed by Postgres
/// constraint name. Add an entry here whenever a new UNIQUE constraint is
/// added to the schema.
fn unique_violation_message(constraint: Option<&str>) -> Option<&'static str> {
    match constraint? {
        "customers_email_key" => Some("A customer with this email already exists."),
        "products_sku_key" => Some("A product with this SKU already exists."),
        _ => None,
    }
}

impl AppError {
    /// The HTTP status and the client-safe message for this error. Database
    /// errors are logged here and replaced with a generic message, so raw
    /// driver errors never reach the browser.
    pub fn status_and_message(&self) -> (StatusCode, String) {
        match self {
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::NotFound(resource) => {
                (StatusCode::NOT_FOUND, format!("{resource} not found."))
            }
            AppError::Database(err) => {
                tracing::error!(error = ?err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong. Please try again.".to_string(),
                )
            }
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(msg) => write!(f, "validation error: {msg}"),
            AppError::NotFound(resource) => write!(f, "{resource} not found"),
            AppError::Database(err) => write!(f, "database error: {err}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Database(err) => Some(err),
            _ => None,
        }
    }
}
