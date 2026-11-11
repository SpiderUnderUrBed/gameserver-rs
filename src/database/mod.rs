
pub mod databasespec;

#[cfg(any(feature = "full-stack", feature = "database"))]
#[path = "pgdatabase.rs"]
mod backend;

// For now I only restrict the json backend for running this without kubernetes
// the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
// and you can use postgres manually with the database feature flag
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[path = "jsondatabase.rs"]
mod backend;

pub mod database {
    pub use super::backend::*;
}