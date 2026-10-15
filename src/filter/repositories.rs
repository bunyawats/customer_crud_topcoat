use sqlx::PgPool;

/// The stored criteria as JSON text, if this session has a filter on `resource`.
pub async fn get(
    pool: &PgPool,
    session: &[u8],
    resource: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT criteria::text AS "criteria!" FROM list_filters
        WHERE session_hash = $1 AND resource = $2
        "#,
        session,
        resource
    )
    .fetch_optional(pool)
    .await
}

pub async fn upsert(
    pool: &PgPool,
    session: &[u8],
    resource: &str,
    criteria: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO list_filters (session_hash, resource, criteria)
        VALUES ($1, $2, $3::text::jsonb)
        ON CONFLICT (session_hash, resource)
        DO UPDATE SET criteria = EXCLUDED.criteria, updated_at = now()
        "#,
        session,
        resource,
        criteria
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear(pool: &PgPool, session: &[u8], resource: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM list_filters WHERE session_hash = $1 AND resource = $2",
        session,
        resource
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes filters last set more than `max_age_secs` ago (abandoned sessions).
pub async fn purge_older_than(pool: &PgPool, max_age_secs: f64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM list_filters WHERE updated_at < now() - make_interval(secs => $1)",
        max_age_secs
    )
    .execute(pool)
    .await?;
    Ok(())
}
