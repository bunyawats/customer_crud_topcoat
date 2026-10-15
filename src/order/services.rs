use sqlx::PgPool;

use crate::bulk::{self, BulkResult};
use crate::customer::services as customer_service;
use crate::error::AppError;
use crate::filter::ListFilter;
use crate::pagination::{self, PAGE_SIZE, Page, PageRequest};
use crate::product::services as product_service;

use super::models::{
    LineItemInput, NewOrder, NewOrderItem, OrderDetail, OrderForm, OrderSummary, STATUSES,
};
use super::repositories as repo;

/// One page of the list under `filter` (an order number, or part of the
/// customer's name), reusing the cached total when `request` carries a
/// `query_id` minted for the same filter.
pub async fn list_page(
    pool: &PgPool,
    request: PageRequest,
    filter: &ListFilter,
) -> Result<Page<OrderSummary>, AppError> {
    let pattern = filter.search.like_pattern();
    let pattern = pattern.as_deref();
    let order_id = filter.search.order_id();
    pagination::fetch_page(
        request,
        &filter.cache_key("orders"),
        async || Ok(repo::count(pool, pattern, order_id).await?),
        async |limit, offset| Ok(repo::list_page(pool, pattern, order_id, limit, offset).await?),
    )
    .await
}

/// How many of `ids` the list hides under `filter` (for the bulk-delete
/// confirm: selected rows outside the current search). Pass ids that
/// still exist: a missing id counts as hidden.
pub async fn count_outside(
    pool: &PgPool,
    ids: &[i64],
    filter: &ListFilter,
) -> Result<usize, AppError> {
    if filter.is_empty() {
        return Ok(0);
    }
    let pattern = filter.search.like_pattern();
    let order_id = filter.search.order_id();
    let matching = repo::count_matching_ids(pool, ids, pattern.as_deref(), order_id).await?;
    Ok(ids
        .len()
        .saturating_sub(usize::try_from(matching).unwrap_or(0)))
}

/// The 1-based page that shows order `id` (e.g. a just-created record).
pub async fn page_of(pool: &PgPool, id: i64) -> Result<i64, AppError> {
    let position = repo::position_of(pool, id)
        .await?
        .ok_or(AppError::NotFound("Order"))?;
    Ok(pagination::page_of_position(position, PAGE_SIZE))
}

pub async fn get(pool: &PgPool, id: i64) -> Result<OrderDetail, AppError> {
    let order = repo::find(pool, id)
        .await?
        .ok_or(AppError::NotFound("Order"))?;
    let items = repo::find_items(pool, id).await?;
    Ok(OrderDetail { order, items })
}

pub async fn create(pool: &PgPool, form: OrderForm) -> Result<i64, AppError> {
    let shape = validate_shape(form)?;
    let input = resolve(pool, shape).await?;
    Ok(repo::create(pool, &input).await?)
}

pub async fn update(pool: &PgPool, id: i64, form: OrderForm) -> Result<(), AppError> {
    let shape = validate_shape(form)?;
    let input = resolve(pool, shape).await?;
    if repo::update(pool, id, &input).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Order"))
    }
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), AppError> {
    if repo::delete(pool, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Order"))
    }
}

/// Deletes each id through [`delete`], so its guards apply per item.
/// Best-effort: see [`bulk::fan_out`].
pub async fn bulk_delete(pool: &PgPool, ids: &[i64]) -> Result<Vec<BulkResult>, AppError> {
    let batch = bulk::prepare_batch(ids)?;
    bulk::fan_out(&batch, |id| delete(pool, id)).await
}

/// Exposed for `customer::services::delete` to check whether a customer has
/// order history before allowing the delete - see that module's delete-guard
/// comment for why this lives here rather than customer reading the `orders`
/// table directly.
pub async fn count_for_customer(pool: &PgPool, customer_id: i64) -> Result<i64, AppError> {
    Ok(repo::count_for_customer(pool, customer_id).await?)
}

/// Exposed for `product::services::delete` to check whether a product is
/// still referenced by any order line item before allowing the delete.
pub async fn count_for_product(pool: &PgPool, product_id: i64) -> Result<i64, AppError> {
    Ok(repo::count_for_product(pool, product_id).await?)
}

/// The syntactically-valid, but not-yet-database-checked, shape of an
/// order submission: a parsed customer id, a known status, and a
/// non-empty list of line items with positive quantities. Kept separate
/// from `resolve` so this part is unit-testable without a database - see
/// the `rust-best-practices` skill's testing conventions.
#[derive(Debug)]
struct OrderShape {
    customer_id: i64,
    status: String,
    items: Vec<LineItemInput>,
}

fn validate_shape(form: OrderForm) -> Result<OrderShape, AppError> {
    let customer_id: i64 = form
        .customer_id
        .trim()
        .parse()
        .map_err(|_| AppError::Validation("Please choose a customer.".into()))?;

    let status = form.status.trim().to_string();
    if !STATUSES.contains(&status.as_str()) {
        return Err(AppError::Validation("Please choose a valid status.".into()));
    }

    let items: Vec<LineItemInput> = serde_json::from_str(&form.items_json)
        .map_err(|_| AppError::Validation("Line items are invalid.".into()))?;
    if items.is_empty() {
        return Err(AppError::Validation("Add at least one line item.".into()));
    }
    if items.iter().any(|item| item.quantity < 1) {
        return Err(AppError::Validation(
            "Line item quantity must be at least 1.".into(),
        ));
    }

    Ok(OrderShape {
        customer_id,
        status,
        items,
    })
}

/// Confirms the customer and every referenced product actually exist, and
/// resolves each line item's current price server-side - a client-supplied
/// price is never trusted, since the browser is exactly the thing an order
/// total needs to be correct against.
///
/// A missing customer/product here means the *order* submission referenced
/// something that doesn't exist (stale form state, tampered payload) - a
/// validation problem with this request, not a "resource not found" in the
/// REST sense - so `customer_service`/`product_service`'s `NotFound` (which
/// would render as 404) is deliberately remapped to `Validation` (422).
async fn resolve(pool: &PgPool, shape: OrderShape) -> Result<NewOrder, AppError> {
    customer_service::get(pool, shape.customer_id)
        .await
        .map_err(|_| AppError::Validation("Selected customer no longer exists.".into()))?;

    let mut items = Vec::with_capacity(shape.items.len());
    for item in shape.items {
        let product = product_service::get(pool, item.product_id)
            .await
            .map_err(|_| AppError::Validation("Selected product no longer exists.".into()))?;
        items.push(NewOrderItem {
            product_id: item.product_id,
            quantity: item.quantity,
            unit_price_cents: product.price_cents,
        });
    }

    Ok(NewOrder {
        customer_id: shape.customer_id,
        status: shape.status,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(customer_id: &str, status: &str, items_json: &str) -> OrderForm {
        OrderForm {
            customer_id: customer_id.to_string(),
            status: status.to_string(),
            items_json: items_json.to_string(),
        }
    }

    #[test]
    fn accepts_well_formed_input() {
        let shape = validate_shape(form("3", "pending", r#"[{"product_id":1,"quantity":2}]"#))
            .expect("valid input should pass validation");

        assert_eq!(shape.customer_id, 3);
        assert_eq!(shape.status, "pending");
        assert_eq!(shape.items.len(), 1);
        assert_eq!(shape.items[0].product_id, 1);
        assert_eq!(shape.items[0].quantity, 2);
    }

    #[test]
    fn rejects_non_numeric_customer_id() {
        let err = validate_shape(form(
            "nope",
            "pending",
            r#"[{"product_id":1,"quantity":1}]"#,
        ))
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_unknown_status() {
        let err = validate_shape(form(
            "3",
            "not-a-status",
            r#"[{"product_id":1,"quantity":1}]"#,
        ))
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_empty_items() {
        let err = validate_shape(form("3", "pending", "[]")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_malformed_items_json() {
        let err = validate_shape(form("3", "pending", "not json")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_zero_quantity() {
        let err =
            validate_shape(form("3", "pending", r#"[{"product_id":1,"quantity":0}]"#)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
