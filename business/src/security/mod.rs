//! Security primitives shared by every domain module.
//!
//! - [`redact`]: turns parsed config into [`crate::model::ConfigValue`] with secrets hidden, and
//!   masks secrets in free text.
//! - [`deny`]: the never-read list and [`SafeReader`], the only way business code reads files.
//! - [`paths`]: canonicalization, containment and argv charset checks.

pub mod deny;
pub mod paths;
pub mod redact;
mod rules;

pub use deny::{DenyReason, SafeReader, is_denied};
pub use paths::{canonical_within, is_safe_arg, validate_project_dir};
pub use redact::{RedactionContext, Redactor, redact_text};
