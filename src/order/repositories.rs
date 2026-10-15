use sqlx::PgPool;

use super::models::{NewOrder, NewOrderItem, OrderItemDetail, OrderSummary};

pub async fn list(pool: &PgPool) -> Result<Vec<OrderSummary>, sqlx::Error> {
    sqlx::query_as!(
        OrderSummary,
        r#"
        SELECT
            o.id,
            o.customer_id,
            c.first_name || ' ' || c.last_name AS "customer_name!",
            o.status,
            COALESCE(SUM(oi.quantity * oi.unit_price_cents), 0)::bigint AS "total_cents!",
            o.created_at,
            o.updated_at
        FROM orders o
        JOIN customers c ON c.id = o.customer_id
        LEFT JOIN order_items oi ON oi.order_id = o.id
        GROUP BY o.id, c.first_name, c.last_name
        ORDER BY o.created_at DESC, o.id DESC
        "#
    )
    .fetch_all(pool)
    .await
}

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<OrderSummary>, sqlx::Error> {
    sqlx::query_as!(
        OrderSummary,
        r#"
        SELECT
            o.id,
            o.customer_id,
            c.first_name || ' ' || c.last_name AS "customer_name!",
            o.status,
            COALESCE(SUM(oi.quantity * oi.unit_price_cents), 0)::bigint AS "total_cents!",
            o.created_at,
            o.updated_at
        FROM orders o
        JOIN customers c ON c.id = o.customer_id
        LEFT JOIN order_items oi ON oi.order_id = o.id
        WHERE o.id = $1
        GROUP BY o.id, c.first_name, c.last_name
        "#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn find_items(pool: &PgPool, order_id: i64) -> Result<Vec<OrderItemDetail>, sqlx::Error> {
    sqlx::query_as!(
        OrderItemDetail,
        r#"
        SELECT oi.id, oi.product_id, p.name AS product_name, oi.quantity, oi.unit_price_cents
        FROM order_items oi
        JOIN products p ON p.id = oi.product_id
        WHERE oi.order_id = $1
        ORDER BY oi.id
        "#,
        order_id
    )
    .fetch_all(pool)
    .await
}

/// Creates an order and its line items atomically - a partial write (order
/// with no items, or items with no order) would be a broken order, so both
/// inserts share one transaction rather than being separate pool calls.
pub async fn create(pool: &PgPool, input: &NewOrder) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let order_id = sqlx::query_scalar!(
        r#"INSERT INTO orders (customer_id, status) VALUES ($1, $2) RETURNING id"#,
        input.customer_id,
        input.status
    )
    .fetch_one(&mut *tx)
    .await?;

    for item in &input.items {
        insert_item(&mut tx, order_id, item).await?;
    }

    tx.commit().await?;
    Ok(order_id)
}

/// Updates an order's header fields and replaces its full set of line
/// items. Replacing rather than diffing keeps this simple: the edit form
/// always submits the complete desired item list, so the source of truth
/// is "what's in the form now," not "what changed since last save."
/// Returns `false` if no order had this id.
pub async fn update(pool: &PgPool, id: i64, input: &NewOrder) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let result = sqlx::query!(
        r#"UPDATE orders SET customer_id = $2, status = $3, updated_at = now() WHERE id = $1"#,
        id,
        input.customer_id,
        input.status
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }

    sqlx::query!("DELETE FROM order_items WHERE order_id = $1", id)
        .execute(&mut *tx)
        .await?;

    for item in &input.items {
        insert_item(&mut tx, id, item).await?;
    }

    tx.commit().await?;
    Ok(true)
}

async fn insert_item(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: i64,
    item: &NewOrderItem,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO order_items (order_id, product_id, quantity, unit_price_cents)
        VALUES ($1, $2, $3, $4)
        "#,
        order_id,
        item.product_id,
        item.quantity,
        item.unit_price_cents
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Returns `true` if a row was deleted, `false` if no order had this id.
/// Its `order_items` rows are removed automatically via `ON DELETE CASCADE`.
pub async fn delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM orders WHERE id = $1", id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Number of orders placed for this customer - used by `customer::services`
/// to block deleting a customer who still has order history.
pub async fn count_for_customer(pool: &PgPool, customer_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM orders WHERE customer_id = $1"#,
        customer_id
    )
    .fetch_one(pool)
    .await
}

/// Number of order line items referencing this product - used by
/// `product::services` to block deleting a product still used in orders.
pub async fn count_for_product(pool: &PgPool, product_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM order_items WHERE product_id = $1"#,
        product_id
    )
    .fetch_one(pool)
    .await
}
