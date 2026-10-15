use sqlx::PgPool;

use super::models::{NewProduct, Product};

pub async fn list(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, name, sku, price_cents, description, created_at, updated_at
        FROM products
        ORDER BY name
        "#
    )
    .fetch_all(pool)
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
