// SQLx migrations and queries — T-002, T-003
pub mod connection;
pub mod migrations;

pub use connection::{create_encrypted_db, open_encrypted_db, verify_encrypted_db, DatabaseError};
pub use migrations::{run_migrations, MigrationError};
