//! Server-side multi-select state for the list screens: which rows a browser
//! session has ticked, per resource. Keyed by the session token's hash (raw
//! bytes here, so no Topcoat types), never by anything the client echoes.
pub mod models;
pub mod repositories;
pub mod services;
