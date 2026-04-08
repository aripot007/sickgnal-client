pub mod account;
pub mod ephemeral_keys;
pub mod peers;
pub mod session;
pub mod session_keys;

/// Trait to store and retrieve an object in the database
pub(crate) trait Store<Target: Sized> {
    /// Table name
    const TABLE: &str;

    /// Table schema
    const SCHEMA: &str;

    /// Sql statement executed after all tables are created
    const POST_CREATE_SQL: &str = "";

    /// Primary key type
    type Id;
}
