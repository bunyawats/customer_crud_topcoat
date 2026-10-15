use std::time::Duration;

use sqlx::PgPool;

use super::models::ListFilter;
use super::repositories as repo;
use crate::error::AppError;
use crate::selection::models::Resource;

/// Filters not set for this long are purged on the next write. They only
/// bound abandoned rows: a live session overwrites its own on every search.
pub const MAX_FILTER_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// `session`'s criteria for `resource`'s list. No row means no filter, and
/// so does a stored row that no longer deserializes: a bad filter must never
/// break the page or hide rows, so it fails open to "show everything".
pub async fn get(
    pool: &PgPool,
    session: &[u8],
    resource: Resource,
) -> Result<ListFilter, AppError> {
    let Some(criteria) = repo::get(pool, session, resource.as_str()).await? else {
        return Ok(ListFilter::default());
    };
    Ok(serde_json::from_str(&criteria).unwrap_or_else(|err| {
        tracing::warn!(error = %err, resource = resource.as_str(), "ignoring unreadable list filter");
        ListFilter::default()
    }))
}

/// Stores `filter` as `session`'s criteria for `resource`. An empty filter
/// deletes the row instead, so "no filter" has one representation.
pub async fn set(
    pool: &PgPool,
    session: &[u8],
    resource: Resource,
    filter: &ListFilter,
) -> Result<(), AppError> {
    repo::purge_older_than(pool, MAX_FILTER_AGE.as_secs_f64()).await?;
    if filter.is_empty() {
        return clear(pool, session, resource).await;
    }
    let criteria = serde_json::to_string(filter).expect("ListFilter serializes");
    Ok(repo::upsert(pool, session, resource.as_str(), &criteria).await?)
}

pub async fn clear(pool: &PgPool, session: &[u8], resource: Resource) -> Result<(), AppError> {
    Ok(repo::clear(pool, session, resource.as_str()).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Search;

    const ALICE: &[u8] = &[1; 32];
    const BOB: &[u8] = &[2; 32];

    fn searching(term: &str) -> ListFilter {
        ListFilter {
            search: Search::new(term).unwrap(),
        }
    }

    #[sqlx::test]
    async fn set_get_overwrite_and_clear(pool: PgPool) {
        let customers = Resource::Customers;
        assert_eq!(
            get(&pool, ALICE, customers).await.unwrap(),
            ListFilter::default()
        );

        set(&pool, ALICE, customers, &searching("ada"))
            .await
            .unwrap();
        assert_eq!(
            get(&pool, ALICE, customers).await.unwrap(),
            searching("ada")
        );

        set(&pool, ALICE, customers, &searching("bob"))
            .await
            .unwrap();
        assert_eq!(
            get(&pool, ALICE, customers).await.unwrap(),
            searching("bob")
        );

        clear(&pool, ALICE, customers).await.unwrap();
        assert_eq!(
            get(&pool, ALICE, customers).await.unwrap(),
            ListFilter::default()
        );
    }

    #[sqlx::test]
    async fn an_empty_filter_deletes_the_row(pool: PgPool) {
        set(&pool, ALICE, Resource::Products, &searching("sku"))
            .await
            .unwrap();
        set(&pool, ALICE, Resource::Products, &ListFilter::default())
            .await
            .unwrap();
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM list_filters")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[sqlx::test]
    async fn sessions_and_resources_are_isolated(pool: PgPool) {
        set(&pool, ALICE, Resource::Customers, &searching("ada"))
            .await
            .unwrap();
        set(&pool, BOB, Resource::Customers, &searching("bob"))
            .await
            .unwrap();
        set(&pool, ALICE, Resource::Orders, &searching("#3"))
            .await
            .unwrap();

        assert_eq!(
            get(&pool, ALICE, Resource::Customers).await.unwrap(),
            searching("ada")
        );
        assert_eq!(
            get(&pool, BOB, Resource::Customers).await.unwrap(),
            searching("bob")
        );
        assert_eq!(
            get(&pool, ALICE, Resource::Orders).await.unwrap(),
            searching("#3")
        );
        assert_eq!(
            get(&pool, BOB, Resource::Orders).await.unwrap(),
            ListFilter::default()
        );
    }

    #[sqlx::test]
    async fn unreadable_criteria_fail_open(pool: PgPool) {
        let too_long = format!(r#"{{"search":"{}"}}"#, "a".repeat(500));
        for criteria in [r#"{"search":42}"#, too_long.as_str()] {
            sqlx::query(
                "INSERT INTO list_filters (session_hash, resource, criteria)
                 VALUES ($1, 'customers', $2::jsonb)
                 ON CONFLICT (session_hash, resource) DO UPDATE SET criteria = EXCLUDED.criteria",
            )
            .bind(ALICE)
            .bind(criteria)
            .execute(&pool)
            .await
            .unwrap();
            assert_eq!(
                get(&pool, ALICE, Resource::Customers).await.unwrap(),
                ListFilter::default(),
                "{criteria}"
            );
        }
    }

    #[sqlx::test]
    async fn writes_purge_abandoned_filters(pool: PgPool) {
        set(&pool, BOB, Resource::Customers, &searching("old"))
            .await
            .unwrap();
        sqlx::query("UPDATE list_filters SET updated_at = now() - interval '2 days'")
            .execute(&pool)
            .await
            .unwrap();
        set(&pool, ALICE, Resource::Customers, &searching("new"))
            .await
            .unwrap();
        assert_eq!(
            get(&pool, BOB, Resource::Customers).await.unwrap(),
            ListFilter::default()
        );
        assert_eq!(
            get(&pool, ALICE, Resource::Customers).await.unwrap(),
            searching("new")
        );
    }
}
