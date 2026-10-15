use sqlx::PgPool;

use super::models::{NewProduct, Product};

pub async fn list(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, name, sku, price_cents, description, created_at, updated_at
        FROM products
        ORDER BY name, id
        "#
    )
    .fetch_all(pool)
    .await
}

/// Products matching `pattern` (an escaped `ILIKE` pattern from
/// `filter::Search::like_pattern`) on name, SKU or description; all of them
/// when `None`.
pub async fn count(pool: &PgPool, pattern: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!" FROM products
        WHERE $1::text IS NULL
           OR name ILIKE $1
           OR sku ILIKE $1
           OR description ILIKE $1
        "#,
        pattern
    )
    .fetch_one(pool)
    .await
}

/// How many of `ids` match `pattern` the way [`count`] does.
pub async fn count_matching_ids(
    pool: &PgPool,
    ids: &[i64],
    pattern: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!" FROM products
        WHERE id = ANY($1)
          AND (name ILIKE $2
               OR sku ILIKE $2
               OR description ILIKE $2)
        "#,
        ids,
        pattern
    )
    .fetch_one(pool)
    .await
}

/// One page of [`list`]'s order, filtered like [`count`]. The `id`
/// tiebreaker keeps `OFFSET` paging stable when names tie.
pub async fn list_page(
    pool: &PgPool,
    pattern: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, name, sku, price_cents, description, created_at, updated_at
        FROM products
        WHERE $1::text IS NULL
           OR name ILIKE $1
           OR sku ILIKE $1
           OR description ILIKE $1
        ORDER BY name, id
        LIMIT $2 OFFSET $3
        "#,
        pattern,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
}

/// 0-based position of product `id` in list order, or `None` if it doesn't
/// exist.
pub async fn position_of(pool: &PgPool, id: i64) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT (
            SELECT COUNT(*) FROM products p
            WHERE (p.name, p.id) < (t.name, t.id)
        ) AS "position!"
        FROM products t
        WHERE t.id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, name, sku, price_cents, description, created_at, updated_at
        FROM products
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn create(pool: &PgPool, input: &NewProduct) -> Result<Product, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        INSERT INTO products (name, sku, price_cents, description)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, sku, price_cents, description, created_at, updated_at
        "#,
        input.name,
        input.sku,
        input.price_cents,
        input.description
    )
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    input: &NewProduct,
) -> Result<Option<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        UPDATE products
        SET name = $2, sku = $3, price_cents = $4, description = $5, updated_at = now()
        WHERE id = $1
        RETURNING id, name, sku, price_cents, description, created_at, updated_at
        "#,
        id,
        input.name,
        input.sku,
        input.price_cents,
        input.description
    )
    .fetch_optional(pool)
    .await
}

/// Returns `true` if a row was deleted, `false` if no product had this id.
pub async fn delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM products WHERE id = $1", id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
