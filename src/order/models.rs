use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Order {
    pub id: i64,
    pub customer_id: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub quantity: i32,
    pub unit_price_cents: i32,
}

/// An order flattened with the customer's name and the computed total, via
/// a join+aggregate query. Used for both the list view and the order
/// detail/edit view's header.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderSummary {
    pub id: i64,
    pub customer_id: i64,
    pub customer_name: String,
    pub status: String,
    pub total_cents: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single line item joined with its product's name, for detail/edit
/// views.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderItemDetail {
    pub id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub quantity: i32,
    pub unit_price_cents: i32,
}

/// An order plus its line items, assembled by the repository layer from
/// two queries.
#[derive(Debug, Clone)]
pub struct OrderDetail {
    pub order: OrderSummary,
    pub items: Vec<OrderItemDetail>,
}

/// The allowed values for `Order::status` / `OrderForm::status`.
pub const STATUSES: [&str; 4] = ["pending", "paid", "shipped", "cancelled"];

/// Raw form input, before validation. `items_json` is a JSON-encoded
/// `Vec<LineItemInput>` - a plain HTML form can't natively submit parallel
/// arrays of (product_id, quantity) pairs (`serde_urlencoded` does not
/// zip repeated same-named fields into aligned `Vec`s - verified directly,
/// it errors), so the line-items UI keeps this hidden field in sync via JS
/// instead. See `static/app.js`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderForm {
    pub customer_id: String,
    pub status: String,
    pub items_json: String,
}

/// One line item as submitted by the client: just what the browser knows.
/// The service layer resolves `product_id` to a current price server-side
/// rather than trusting a client-submitted price.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LineItemInput {
    pub product_id: i64,
    pub quantity: i32,
}

/// Validated, owned data ready for persistence. Only the service layer
/// constructs this, after `OrderForm` has passed validation and each
/// line item's product existence/current price has been resolved.
#[derive(Debug, Clone)]
pub struct NewOrder {
    pub customer_id: i64,
    pub status: String,
    pub items: Vec<NewOrderItem>,
}

#[derive(Debug, Clone)]
pub struct NewOrderItem {
    pub product_id: i64,
    pub quantity: i32,
    pub unit_price_cents: i32,
}
