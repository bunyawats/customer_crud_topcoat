use sqlx::PgPool;

use super::models::{Customer, NewCustomer};

pub async fn list(pool: &PgPool) -> Result<Vec<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        SELECT id, first_name, last_name, email, phone, created_at, updated_at
        FROM customers
        ORDER BY last_name, first_name, id
        "#
    )
    .fetch_all(pool)
    .await
}

/// Customers matching `pattern` (an escaped `ILIKE` pattern from
/// `filter::Search::like_pattern`) on full name, email or phone; all of them
/// when `None`.
pub async fn count(pool: &PgPool, pattern: Option<&str>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!" FROM customers
        WHERE $1::text IS NULL
           OR first_name || ' ' || last_name ILIKE $1
           OR email ILIKE $1
           OR phone ILIKE $1
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
        SELECT COUNT(*) AS "count!" FROM customers
        WHERE id = ANY($1)
          AND (first_name || ' ' || last_name ILIKE $2
               OR email ILIKE $2
               OR phone ILIKE $2)
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
) -> Result<Vec<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        SELECT id, first_name, last_name, email, phone, created_at, updated_at
        FROM customers
        WHERE $1::text IS NULL
           OR first_name || ' ' || last_name ILIKE $1
           OR email ILIKE $1
           OR phone ILIKE $1
        ORDER BY last_name, first_name, id
        LIMIT $2 OFFSET $3
        "#,
        pattern,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
}

/// 0-based position of customer `id` in list order, or `None` if it doesn't
/// exist. Row comparison matches the `ORDER BY` above column for column.
pub async fn position_of(pool: &PgPool, id: i64) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT (
            SELECT COUNT(*) FROM customers c
            WHERE (c.last_name, c.first_name, c.id) < (t.last_name, t.first_name, t.id)
        ) AS "position!"
        FROM customers t
        WHERE t.id = $1
        "#,
        id
    )
    .fetch_optional(pool)
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
