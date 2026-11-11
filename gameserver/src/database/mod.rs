pub mod databasespec;

pub mod database {
    #[cfg(feature = "postgres")]
    include!("pgdatabase.rs");
    
    #[cfg(not(feature = "postgres"))]
    include!("jsondatabase.rs");
}
