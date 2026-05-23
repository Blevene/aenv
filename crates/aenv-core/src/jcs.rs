//! RFC 8785 JSON Canonicalization Scheme (JCS).
//!
//! Phase 5 introduces a deterministic JSON serialization for use as a
//! hash-input transformation. The implementation walks `serde_json::Value`
//! and writes a `String` per RFC 8785: object keys sorted by UTF-16 code
//! unit, numbers in shortest ECMAScript `JSON.stringify` form, strings
//! minimally escaped, no extraneous whitespace.
//!
//! Entry point lands in Task 2. This file exists so `lib.rs` can re-export
//! `pub mod jcs;` without a chain of dependent diffs.

#![allow(dead_code)] // Filled in by Task 2.
