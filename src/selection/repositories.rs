use sqlx::PgPool;

/// Selected ids in the order they were ticked.
pub async fn get(pool: &PgPool, session: &[u8], resource: &str) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT item_id FROM selected_items
        WHERE session_hash = $1 AND resource = $2
        ORDER BY selected_at, item_id
        "#,
        session,
        resource
    )
    .fetch_all(pool)
    .await
}

pub async fn add(
    pool: &PgPool,
    session: &[u8],
    resource: &str,
    ids: &[i64],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO selected_items (session_hash, resource, item_id)
        SELECT $1, $2, id FROM UNNEST($3::BIGINT[]) AS id
        ON CONFLICT DO NOTHING
        "#,
        session,
        resource,
        ids
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove(
    pool: &PgPool,
    session: &[u8],
    resource: &str,
    ids: &[i64],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM selected_items
        WHERE session_hash = $1 AND resource = $2 AND item_id = ANY($3)
        "#,
        session,
        resource,
        ids
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear(pool: &PgPool, session: &[u8], resource: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM selected_items WHERE session_hash = $1 AND resource = $2",
        session,
        resource
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Drops a deleted row from every session's selection.
pub async fn forget_item(pool: &PgPool, resource: &str, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM selected_items WHERE resource = $1 AND item_id = $2",
        resource,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes selections ticked more than `max_age_secs` ago (abandoned tabs).
pub async fn purge_older_than(pool: &PgPool, max_age_secs: f64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM selected_items WHERE selected_at < now() - make_interval(secs => $1)",
        max_age_secs
    )
    .execute(pool)
    .await?;
    Ok(())
}
