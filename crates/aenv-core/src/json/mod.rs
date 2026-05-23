//! Typed response shapes for every `--json` flag.
//!
//! Each command crate constructs an instance and prints
//! `serde_json::to_string_pretty(&shape)`. Schemas are locked with
//! insta snapshot tests in `tests/json_snapshots.rs` (Tasks 8–14).
//!
//! The struct field names ARE the public schema. Renaming a field is
//! a breaking change per PRD R-77 (qualified-name fields) and R-85
//! (resolved-hash format).

pub mod adapter;
pub mod diff;
pub mod doctor;
pub mod get;
pub mod list;
pub mod skill;
pub mod status;
pub mod which;

pub use adapter::AdapterEntryJson;
pub use diff::{DriftReport, StructuralDiff};
pub use doctor::DoctorReportJson;
pub use get::GetReport;
pub use list::ListEntry;
pub use skill::SkillEntry;
pub use status::{ParameterEntryJson, StatusReport};
pub use which::WhichReport;
