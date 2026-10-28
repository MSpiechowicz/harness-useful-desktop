//! Security primitives shared by every domain module.
//!
//! - [`redact`]: turns parsed config into [`crate::model::ConfigValue`] with secrets hidden, and
//!   masks secrets in free text.
//! - [`bounded_redacted`] and [`tail_redacted`] (and the `_masked` variants that also mask a
//!   secret first): redacted, size-capped text for the webview.
//! - [`deny`]: the never-read list and [`SafeReader`], the only way business code reads files.
//! - [`paths`]: canonicalization, containment and argv charset checks.

pub mod deny;
pub mod paths;
pub mod redact;
mod rules;
mod text;

pub use deny::{DenyReason, SafeReader, is_denied};
pub use paths::{canonical_within, is_safe_arg, validate_project_dir};
pub use redact::{RedactionContext, Redactor, mask_secret, redact_text};
pub use text::{bounded_masked, bounded_redacted, tail_masked, tail_redacted};
