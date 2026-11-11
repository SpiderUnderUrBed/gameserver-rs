
pub mod databasespec;
pub mod database {
    // For now I only restrict the json backend for running this without kubernetes
    // the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
    // and you can use postgres manually with the database feature flag
    #[cfg(any(feature = "full-stack", feature = "database"))]
    include!("pgdatabase.rs");

    #[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
    include!("database/jsondatabase.rs");
}