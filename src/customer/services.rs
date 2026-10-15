use sqlx::PgPool;

use super::models::{Customer, CustomerForm, NewCustomer};
use super::repositories as repo;
use crate::bulk::{self, BulkResult};
use crate::error::AppError;
use crate::filter::ListFilter;
use crate::pagination::{self, PAGE_SIZE, Page, PageRequest};

pub async fn list(pool: &PgPool) -> Result<Vec<Customer>, AppError> {
    Ok(repo::list(pool).await?)
}

/// One page of the list under `filter`, reusing the cached total when
/// `request` carries a `query_id` minted for the same filter.
pub async fn list_page(
    pool: &PgPool,
    request: PageRequest,
    filter: &ListFilter,
) -> Result<Page<Customer>, AppError> {
    let pattern = filter.search.like_pattern();
    let pattern = pattern.as_deref();
    pagination::fetch_page(
        request,
        &filter.cache_key("customers"),
        async || Ok(repo::count(pool, pattern).await?),
        async |limit, offset| Ok(repo::list_page(pool, pattern, limit, offset).await?),
    )
    .await
}

/// How many of `ids` the list hides under `filter` (for the bulk-delete
/// confirm: selected rows outside the current search). Pass ids that
/// still exist: a missing id counts as hidden.
pub async fn count_outside(
    pool: &PgPool,
    ids: &[i64],
    filter: &ListFilter,
) -> Result<usize, AppError> {
    let Some(pattern) = filter.search.like_pattern() else {
        return Ok(0);
    };
    let matching = repo::count_matching_ids(pool, ids, &pattern).await?;
    Ok(ids
        .len()
        .saturating_sub(usize::try_from(matching).unwrap_or(0)))
}

/// The 1-based page that shows customer `id` (e.g. a just-created record).
pub async fn page_of(pool: &PgPool, id: i64) -> Result<i64, AppError> {
    let position = repo::position_of(pool, id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    Ok(pagination::page_of_position(position, PAGE_SIZE))
}

pub async fn get(pool: &PgPool, id: i64) -> Result<Customer, AppError> {
    repo::find(pool, id)
        .await?
        .ok_or(AppError::NotFound("Customer"))
}

pub async fn create(pool: &PgPool, form: CustomerForm) -> Result<Customer, AppError> {
    let input = validate(form)?;
    Ok(repo::create(pool, &input).await?)
}

pub async fn update(pool: &PgPool, id: i64, form: CustomerForm) -> Result<Customer, AppError> {
    let input = validate(form)?;
    repo::update(pool, id, &input)
        .await?
        .ok_or(AppError::NotFound("Customer"))
}

/// Refuses to delete a customer who still has order history - a DB-level
/// foreign key would also stop this, but as a generic 500 with no useful
/// message. Checking here means the failure comes back as a clear
/// `AppError::Validation` instead.
pub async fn delete(pool: &PgPool, id: i64) -> Result<(), AppError> {
    let order_count = crate::order::services::count_for_customer(pool, id).await?;
    if order_count > 0 {
        let (orders, references) = if order_count == 1 {
            ("order", "references")
        } else {
            ("orders", "reference")
        };
        return Err(AppError::Validation(format!(
            "Cannot delete this customer: {order_count} {orders} still {references} them."
        )));
    }

    if repo::delete(pool, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Customer"))
    }
}

/// Deletes each id through [`delete`], so its guards apply per item.
/// Best-effort: see [`bulk::fan_out`].
pub async fn bulk_delete(pool: &PgPool, ids: &[i64]) -> Result<Vec<BulkResult>, AppError> {
    let batch = bulk::prepare_batch(ids)?;
    bulk::fan_out(&batch, |id| delete(pool, id)).await
}

/// Domain validation for customer input. Kept separate from `NewCustomer`
/// construction so it's easy to unit test without a database.
fn validate(form: CustomerForm) -> Result<NewCustomer, AppError> {
    let first_name = form.first_name.trim().to_string();
    let last_name = form.last_name.trim().to_string();
    let email = form.email.trim().to_string();
    let phone = form.phone.trim().to_string();

    if first_name.is_empty() {
        return Err(AppError::Validation("First name is required.".into()));
    }
    if last_name.is_empty() {
        return Err(AppError::Validation("Last name is required.".into()));
    }
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::Validation("A valid email is required.".into()));
    }

    Ok(NewCustomer {
        first_name,
        last_name,
        email,
        phone: if phone.is_empty() { None } else { Some(phone) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(first: &str, last: &str, email: &str, phone: &str) -> CustomerForm {
        CustomerForm {
            first_name: first.to_string(),
            last_name: last.to_string(),
            email: email.to_string(),
            phone: phone.to_string(),
        }
    }

    #[test]
    fn accepts_well_formed_input_and_trims_whitespace() {
        let input = validate(form(
            " Ada ",
            " Lovelace ",
            " ada@example.com ",
            " 555-1234 ",
        ))
        .expect("valid input should pass validation");

        assert_eq!(input.first_name, "Ada");
        assert_eq!(input.last_name, "Lovelace");
        assert_eq!(input.email, "ada@example.com");
        assert_eq!(input.phone.as_deref(), Some("555-1234"));
    }

    #[test]
    fn blank_phone_becomes_none() {
        let input = validate(form("Ada", "Lovelace", "ada@example.com", "   ")).unwrap();
        assert_eq!(input.phone, None);
    }

    #[test]
    fn rejects_missing_first_name() {
        let err = validate(form("  ", "Lovelace", "ada@example.com", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_missing_last_name() {
        let err = validate(form("Ada", "  ", "ada@example.com", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_email_without_at_sign() {
        let err = validate(form("Ada", "Lovelace", "not-an-email", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_blank_email() {
        let err = validate(form("Ada", "Lovelace", "   ", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
