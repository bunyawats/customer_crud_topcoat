use sqlx::PgPool;

use super::models::{Customer, CustomerForm, NewCustomer};
use super::repositories as repo;
use crate::error::AppError;

pub async fn list(pool: &PgPool) -> Result<Vec<Customer>, AppError> {
    Ok(repo::list(pool).await?)
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
