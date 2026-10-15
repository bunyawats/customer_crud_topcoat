//! Bulk actions as a fan-out over an existing single-item action (the
//! `list-pagination-bulk-actions` skill's rule): every guard the single-item
//! function has, such as "customer still has orders", applies per item with
//! nothing to keep in sync. Best-effort: one item failing doesn't stop the
//! rest, and each gets its own result.

use crate::error::AppError;

/// Most ids one bulk action may touch; checked before anything runs.
pub const MAX_BATCH: usize = 100;

/// The outcome for one id. `error` is the user-facing message of the
/// single-item action's `AppError::Validation` / `NotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkResult {
    pub id: i64,
    pub error: Option<String>,
}

impl BulkResult {
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
}

/// De-dupes `ids` (keeping first-seen order), and rejects an empty batch or
/// one over [`MAX_BATCH`].
pub fn prepare_batch(ids: &[i64]) -> Result<Vec<i64>, AppError> {
    let mut seen = std::collections::HashSet::new();
    let batch: Vec<i64> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
    if batch.is_empty() {
        return Err(AppError::Validation("Nothing is selected.".into()));
    }
    if batch.len() > MAX_BATCH {
        return Err(AppError::Validation(format!(
            "Select at most {MAX_BATCH} rows at a time ({} selected).",
            batch.len()
        )));
    }
    Ok(batch)
}

/// Runs `action` for each id of a prepared batch, one after another (a small
/// pool and per-item guard queries make concurrency not worth it here).
/// Only the documented per-item failures (`Validation`, `NotFound`) become
/// failed results; a `Database` error is a real fault and aborts the batch.
///
/// `action` is a plain `Fn` returning a future rather than an `AsyncFn`: an
/// async closure's per-call future borrows the closure for a higher-ranked
/// lifetime, which makes the handler future fail Topcoat's `Send` bound.
pub async fn fan_out<F, Fut>(batch: &[i64], action: F) -> Result<Vec<BulkResult>, AppError>
where
    F: Fn(i64) -> Fut,
    Fut: Future<Output = Result<(), AppError>>,
{
    let mut results = Vec::with_capacity(batch.len());
    for &id in batch {
        let error = match action(id).await {
            Ok(()) => None,
            Err(AppError::Validation(message)) => Some(message),
            Err(err @ AppError::NotFound(_)) => Some(err.status_and_message().1),
            Err(err @ AppError::Database(_)) => return Err(err),
        };
        results.push(BulkResult { id, error });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_is_deduped_in_order() {
        assert_eq!(prepare_batch(&[3, 1, 3, 2, 1]).unwrap(), vec![3, 1, 2]);
    }

    #[test]
    fn empty_and_oversized_batches_are_rejected() {
        assert!(matches!(prepare_batch(&[]), Err(AppError::Validation(_))));
        let too_many: Vec<i64> = (0..=MAX_BATCH as i64).collect();
        assert!(matches!(
            prepare_batch(&too_many),
            Err(AppError::Validation(_))
        ));
        let at_cap: Vec<i64> = (0..MAX_BATCH as i64).collect();
        assert_eq!(prepare_batch(&at_cap).unwrap().len(), MAX_BATCH);
        // The cap applies after de-duping.
        assert!(prepare_batch(&vec![7; MAX_BATCH * 2]).is_ok());
    }

    #[tokio::test]
    async fn per_item_failures_are_reported_and_the_rest_still_run() {
        let results = fan_out(&[1, 2, 3], |id| async move {
            match id {
                2 => Err(AppError::Validation("Customer still has orders.".into())),
                3 => Err(AppError::NotFound("Customer")),
                _ => Ok(()),
            }
        })
        .await
        .unwrap();

        assert!(results[0].ok());
        assert_eq!(
            results[1].error.as_deref(),
            Some("Customer still has orders.")
        );
        assert_eq!(results[2].error.as_deref(), Some("Customer not found."));
    }

    #[tokio::test]
    async fn a_database_error_aborts_the_batch() {
        let result = fan_out(&[1, 2], |_| async {
            Err(AppError::Database(sqlx::Error::PoolTimedOut))
        })
        .await;
        assert!(matches!(result, Err(AppError::Database(_))));
    }
}
