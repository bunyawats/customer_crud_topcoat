//! Customer / product / order ledger on Topcoat + htmx + SQLx/PostgreSQL.
//!
//! Two trees, on purpose:
//!
//! - **Domain** (`customer`, `product`, `order`): feature-first modules,
//!   each layered `models` → `repositories` → `services`. No HTTP types.
//! - **Web** (`app`): Topcoat's module-routed tree, where each module is a
//!   URL segment (`app::customers::customer_id::edit` serves
//!   `/customers/{customer_id}/edit`). Pages, routes and the `view!`
//!   components that replace the old Askama templates live here. Shared
//!   markup lives in `ui`.

pub mod app;
pub mod bulk;
pub mod customer;
pub mod error;
pub mod order;
pub mod pagination;
pub mod product;
pub mod selection;
pub mod ui;

pub use app::router;
pub use error::AppError;
