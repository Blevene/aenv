//! Core library for `aenv`.
//!
//! This crate holds all logic, types, and traits. The `aenv-cli` binary is
//! a thin shell that translates command-line invocations into calls against
//! this library. No code below this boundary reads `current_dir()` or
//! environment variables — paths are passed in absolute.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod activate;
pub mod adapter;
pub mod adapters_builtin;
pub mod atomicity;
pub mod deactivate;
pub mod error;
pub mod fs;
pub mod home;
pub mod identity;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod resolve;
pub mod restore;
pub mod state;
pub mod strategy;

pub use error::{AenvError, Result};
