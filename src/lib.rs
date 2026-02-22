/// cclink library crate — exposes internal modules for integration tests.
///
/// All modules are re-exported publicly so that `tests/` integration tests
/// can access crypto, record, and transport functions via `use cclink::crypto::*`.
pub mod crypto;
pub mod error;
pub mod keys;
pub mod record;
pub mod transport;
