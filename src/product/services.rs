use sqlx::PgPool;

use super::models::{NewProduct, Product, ProductForm};
use super::repositories as repo;
use crate::bulk::{self, BulkResult};
use crate::error::AppError;
use crate::pagination::{self, PAGE_SIZE, Page, PageRequest};

pub async fn list(pool: &PgPool) -> Result<Vec<Product>, AppError> {
    Ok(repo::list(pool).await?)
}

/// One page of the list, reusing the cached total when `request` carries a
/// still-valid `query_id`.
pub async fn list_page(pool: &PgPool, request: PageRequest) -> Result<Page<Product>, AppError> {
    pagination::fetch_page(
        request,
        "products",
        async || Ok(repo::count(pool).await?),
        async |limit, offset| Ok(repo::list_page(pool, limit, offset).await?),
    )
    .await
}

/// The 1-based page that shows product `id` (e.g. a just-created record).
pub async fn page_of(pool: &PgPool, id: i64) -> Result<i64, AppError> {
    let position = repo::position_of(pool, id)
        .await?
        .ok_or(AppError::NotFound("Product"))?;
    Ok(pagination::page_of_position(position, PAGE_SIZE))
}

pub async fn get(pool: &PgPool, id: i64) -> Result<Product, AppError> {
    repo::find(pool, id)
        .await?
        .ok_or(AppError::NotFound("Product"))
}

pub async fn create(pool: &PgPool, form: ProductForm) -> Result<Product, AppError> {
    let input = validate(form)?;
    Ok(repo::create(pool, &input).await?)
}

pub async fn update(pool: &PgPool, id: i64, form: ProductForm) -> Result<Product, AppError> {
    let input = validate(form)?;
    repo::update(pool, id, &input)
        .await?
        .ok_or(AppError::NotFound("Product"))
}

/// Refuses to delete a product still referenced by an order line item - a
/// DB-level foreign key would also stop this, but as a generic 500 with no
/// useful message. Checking here means the failure comes back as a clear
/// `AppError::Validation` instead.
pub async fn delete(pool: &PgPool, id: i64) -> Result<(), AppError> {
    let item_count = crate::order::services::count_for_product(pool, id).await?;
    if item_count > 0 {
        let orders = if item_count == 1 { "order" } else { "orders" };
        return Err(AppError::Validation(format!(
            "Cannot delete this product: it is used in {item_count} {orders}."
        )));
    }

    if repo::delete(pool, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Product"))
    }
}

/// Deletes each id through [`delete`], so its guards apply per item.
/// Best-effort: see [`bulk::fan_out`].
pub async fn bulk_delete(pool: &PgPool, ids: &[i64]) -> Result<Vec<BulkResult>, AppError> {
    let batch = bulk::prepare_batch(ids)?;
    bulk::fan_out(&batch, |id| delete(pool, id)).await
}

/// Domain validation for product input. Kept separate from `NewProduct`
/// construction so it's easy to unit test without a database.
fn validate(form: ProductForm) -> Result<NewProduct, AppError> {
    let name = form.name.trim().to_string();
    let sku = form.sku.trim().to_string();
    let description = form.description.trim().to_string();

    if name.is_empty() {
        return Err(AppError::Validation("Name is required.".into()));
    }
    if sku.is_empty() {
        return Err(AppError::Validation("SKU is required.".into()));
    }
    let price_cents = parse_price_cents(&form.price)?;

    Ok(NewProduct {
        name,
        sku,
        price_cents,
        description: if description.is_empty() {
            None
        } else {
            Some(description)
        },
    })
}

/// Parses a dollar-amount string like "19.99" into integer cents.
fn parse_price_cents(raw: &str) -> Result<i32, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Price is required.".into()));
    }

    let dollars: f64 = trimmed
        .parse()
        .map_err(|_| AppError::Validation("Price must be a number.".into()))?;
    if !dollars.is_finite() || dollars < 0.0 {
        return Err(AppError::Validation(
            "Price must be zero or a positive number.".into(),
        ));
    }

    // `price_cents` is an INTEGER column. Check before casting: `as i32`
    // saturates, which would silently store a different price.
    let cents = (dollars * 100.0).round();
    if cents > f64::from(i32::MAX) {
        return Err(AppError::Validation(
            "Price must be at most $21,474,836.47.".into(),
        ));
    }
    Ok(cents as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(name: &str, sku: &str, price: &str, description: &str) -> ProductForm {
        ProductForm {
            name: name.to_string(),
            sku: sku.to_string(),
            price: price.to_string(),
            description: description.to_string(),
        }
    }

    #[test]
    fn accepts_well_formed_input_and_trims_whitespace() {
        let input = validate(form(" Widget ", " WID-1 ", " 19.99 ", " A fine widget. "))
            .expect("valid input should pass validation");

        assert_eq!(input.name, "Widget");
        assert_eq!(input.sku, "WID-1");
        assert_eq!(input.price_cents, 1999);
        assert_eq!(input.description.as_deref(), Some("A fine widget."));
    }

    #[test]
    fn blank_description_becomes_none() {
        let input = validate(form("Widget", "WID-1", "19.99", "   ")).unwrap();
        assert_eq!(input.description, None);
    }

    #[test]
    fn whole_dollar_price_converts_to_cents() {
        let input = validate(form("Widget", "WID-1", "20", "")).unwrap();
        assert_eq!(input.price_cents, 2000);
    }

    #[test]
    fn largest_storable_price_is_accepted() {
        let input = validate(form("Jet", "JET-1", "21474836.47", "")).unwrap();
        assert_eq!(input.price_cents, i32::MAX);
    }

    /// `as i32` saturates, so these used to be stored as $21,474,836.47
    /// without any error.
    #[test]
    fn rejects_price_too_large_to_store() {
        for price in ["21474836.48", "30000000", "1e30"] {
            let err = validate(form("Jet", "JET-1", price, "")).unwrap_err();
            assert!(
                matches!(&err, AppError::Validation(msg) if msg.contains("$21,474,836.47")),
                "{price}: {err:?}"
            );
        }
    }

    #[test]
    fn rejects_missing_name() {
        let err = validate(form("  ", "WID-1", "19.99", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_missing_sku() {
        let err = validate(form("Widget", "  ", "19.99", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_non_numeric_price() {
        let err = validate(form("Widget", "WID-1", "free", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_negative_price() {
        let err = validate(form("Widget", "WID-1", "-5.00", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn rejects_blank_price() {
        let err = validate(form("Widget", "WID-1", "   ", "")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
