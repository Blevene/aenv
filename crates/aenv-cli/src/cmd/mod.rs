//! CLI subcommand handlers.
//!
//! Each handler takes a `Filesystem` reference and a context struct,
//! returning `aenv_core::Result<()>`. The handlers do printing on success.

pub mod adapter;
pub mod create;
pub mod delete;
pub mod list;
pub mod use_;
