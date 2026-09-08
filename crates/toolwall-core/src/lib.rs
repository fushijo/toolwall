//! Shared types and persistence for toolwall.
//!
//! Both the CLI and the GUI are thin shells over this crate. The Lua runtime
//! only ever *reads* the document; every write goes through [`store::Store`],
//! which is also responsible for tripping waywall's hot reload.

pub mod schema;
pub mod store;

pub use schema::{Document, SCHEMA_VERSION};
pub use store::{Problem, Scope, Store, problems, validate};
