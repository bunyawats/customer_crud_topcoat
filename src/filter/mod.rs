//! Server-side search criteria for the list screens: one [`ListFilter`] per
//! browser session and list. Keyed by the session token's hash (raw bytes
//! here, so no Topcoat types). Only the search route writes criteria; every
//! other list request reads them from here, never from the client.
pub mod models;
pub mod repositories;
pub mod services;

pub use models::{ListFilter, Search};
