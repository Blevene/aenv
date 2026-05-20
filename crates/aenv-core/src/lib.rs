//! Core library for `aenv`.
//!
//! This crate holds all logic, types, and traits. The `aenv-cli` binary is
//! a thin shell that translates command-line invocations into calls against
//! this library. No code below this boundary reads `current_dir()` or
//! environment variables — paths are passed in absolute.

#![warn(missing_docs)]
#![warn(clippy::all)]
