use std::time::Duration;

use sqlx::PgPool;

use super::models::Resource;
use super::repositories as repo;
use crate::error::AppError;

/// Selections older than this are purged on the next write. A fresh visit to
/// a list page clears its selection anyway, so this only bounds abandoned rows.
pub const MAX_SELECTION_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The ids `session` has selected on `resource`'s list, in the order ticked.
pub async fn get(pool: &PgPool, session: &[u8], resource: Resource) -> Result<Vec<i64>, AppError> {
    Ok(repo::get(pool, session, resource.as_str()).await?)
}

/// Ticks (`checked`) or unticks `ids`, and returns the selection afterwards.
pub async fn update(
    pool: &PgPool,
    session: &[u8],
    resource: Resource,
    ids: &[i64],
    checked: bool,
) -> Result<Vec<i64>, AppError> {
    repo::purge_older_than(pool, MAX_SELECTION_AGE.as_secs_f64()).await?;
    if checked {
        repo::add(pool, session, resource.as_str(), ids).await?;
    } else {
        repo::remove(pool, session, resource.as_str(), ids).await?;
    }
    get(pool, session, resource).await
}

pub async fn clear(pool: &PgPool, session: &[u8], resource: Resource) -> Result<(), AppError> {
    Ok(repo::clear(pool, session, resource.as_str()).await?)
}

/// Removes a just-deleted row from every session's selection, so no
/// toolbar keeps counting it.
pub async fn forget_item(pool: &PgPool, resource: Resource, id: i64) -> Result<(), AppError> {
    Ok(repo::forget_item(pool, resource.as_str(), id).await?)
}

/// Parses the comma-joined `ids` field the checkboxes send (`"7"` or
/// `"3,7,8"`; htmx can't send a real list, see the htmx4 skill). Blank
/// segments are skipped; anything non-numeric is a bad submission.
pub fn parse_ids(raw: &str) -> Result<Vec<i64>, AppError> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i64>()
                .map_err(|_| AppError::Validation("Invalid selection.".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_and_comma_joined_ids() {
        assert_eq!(parse_ids("7").unwrap(), vec![7]);
        assert_eq!(parse_ids("3,7,8").unwrap(), vec![3, 7, 8]);
        assert_eq!(parse_ids(" 3 , 7 ").unwrap(), vec![3, 7]);
    }

    #[test]
    fn blank_segments_are_skipped() {
        assert!(parse_ids("").unwrap().is_empty());
        assert_eq!(parse_ids("1,,2,").unwrap(), vec![1, 2]);
    }

    #[test]
    fn non_numeric_ids_are_a_validation_error() {
        assert!(matches!(parse_ids("1,abc"), Err(AppError::Validation(_))));
        assert!(matches!(parse_ids("1.5"), Err(AppError::Validation(_))));
    }

    const ALICE: &[u8] = &[1; 32];
    const BOB: &[u8] = &[2; 32];

    #[sqlx::test]
    async fn tick_untick_and_clear(pool: PgPool) {
        let ids = update(&pool, ALICE, Resource::Customers, &[3, 1, 2], true)
            .await
            .unwrap();
        assert_eq!(ids.len(), 3);

        // Ticking again is a no-op, not a duplicate.
        let ids = update(&pool, ALICE, Resource::Customers, &[1], true)
            .await
            .unwrap();
        assert_eq!(ids.len(), 3);

        let ids = update(&pool, ALICE, Resource::Customers, &[1, 3], false)
            .await
            .unwrap();
        assert_eq!(ids, vec![2]);

        clear(&pool, ALICE, Resource::Customers).await.unwrap();
        assert!(
            get(&pool, ALICE, Resource::Customers)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn sessions_and_resources_are_isolated(pool: PgPool) {
        update(&pool, ALICE, Resource::Customers, &[1, 2], true)
            .await
            .unwrap();
        update(&pool, BOB, Resource::Customers, &[9], true)
            .await
            .unwrap();
        update(&pool, ALICE, Resource::Products, &[5], true)
            .await
            .unwrap();

        assert_eq!(
            get(&pool, ALICE, Resource::Customers).await.unwrap(),
            vec![1, 2]
        );
        assert_eq!(get(&pool, BOB, Resource::Customers).await.unwrap(), vec![9]);
        assert_eq!(
            get(&pool, ALICE, Resource::Products).await.unwrap(),
            vec![5]
        );

        clear(&pool, ALICE, Resource::Customers).await.unwrap();
        assert_eq!(get(&pool, BOB, Resource::Customers).await.unwrap(), vec![9]);
        assert_eq!(
            get(&pool, ALICE, Resource::Products).await.unwrap(),
            vec![5]
        );
    }

    #[sqlx::test]
    async fn forgetting_a_deleted_row_drops_it_for_everyone(pool: PgPool) {
        update(&pool, ALICE, Resource::Orders, &[4, 5], true)
            .await
            .unwrap();
        update(&pool, BOB, Resource::Orders, &[4], true)
            .await
            .unwrap();
        update(&pool, BOB, Resource::Products, &[4], true)
            .await
            .unwrap();

        forget_item(&pool, Resource::Orders, 4).await.unwrap();

        assert_eq!(get(&pool, ALICE, Resource::Orders).await.unwrap(), vec![5]);
        assert!(get(&pool, BOB, Resource::Orders).await.unwrap().is_empty());
        assert_eq!(get(&pool, BOB, Resource::Products).await.unwrap(), vec![4]);
    }

    #[sqlx::test]
    async fn old_selections_are_purged_on_write(pool: PgPool) {
        update(&pool, ALICE, Resource::Customers, &[1], true)
            .await
            .unwrap();
        sqlx::query("UPDATE selected_items SET selected_at = now() - interval '2 days'")
            .execute(&pool)
            .await
            .unwrap();

        update(&pool, BOB, Resource::Customers, &[2], true)
            .await
            .unwrap();
        assert!(
            get(&pool, ALICE, Resource::Customers)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(get(&pool, BOB, Resource::Customers).await.unwrap(), vec![2]);
    }
}
