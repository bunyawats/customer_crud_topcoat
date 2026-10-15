use sqlx::PgPool;

use super::models::{Customer, NewCustomer};

pub async fn list(pool: &PgPool) -> Result<Vec<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        SELECT id, first_name, last_name, email, phone, created_at, updated_at
        FROM customers
        ORDER BY last_name, first_name
        "#
    )
    .fetch_all(pool)
    .await
}

pub async fn find(pool: &PgPool, id: i64) -> Result<Option<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        SELECT id, first_name, last_name, email, phone, created_at, updated_at
        FROM customers
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn create(pool: &PgPool, input: &NewCustomer) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        INSERT INTO customers (first_name, last_name, email, phone)
        VALUES ($1, $2, $3, $4)
        RETURNING id, first_name, last_name, email, phone, created_at, updated_at
        "#,
        input.first_name,
        input.last_name,
        input.email,
        input.phone
    )
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    input: &NewCustomer,
) -> Result<Option<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        UPDATE customers
        SET first_name = $2, last_name = $3, email = $4, phone = $5, updated_at = now()
        WHERE id = $1
        RETURNING id, first_name, last_name, email, phone, created_at, updated_at
        "#,
        id,
        input.first_name,
        input.last_name,
        input.email,
        input.phone
    )
    .fetch_optional(pool)
    .await
}

/// Returns `true` if a row was deleted, `false` if no customer had this id.
pub async fn delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM customers WHERE id = $1", id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
