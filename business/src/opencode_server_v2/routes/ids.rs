//! The checked ids OpenCode 2 routes take (see [`super`]): a valid id can never add a path
//! segment or a query.

use std::fmt;
use std::marker::PhantomData;

use serde::{Serialize, Serializer};

use crate::opencode_server::routes::RouteError;

/// Longest accepted id, prefix included (OpenCode 2's are about 30 bytes).
pub const MAX_ID_LEN: usize = 128;

/// What an [`Id`] names: its prefix and a label for errors.
pub trait IdKind {
    /// The prefix every id of this kind starts with.
    const PREFIX: &'static str;

    /// What the id names, for error messages (the value itself is never shown).
    const LABEL: &'static str;
}

/// Marker of session ids (`ses_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {}

/// Marker of message ids (`msg_…`, also inbox items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {}

/// Marker of permission request ids (`per_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {}

/// Marker of form ids (`frm_…`, the questions a tool asks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormKind {}

/// Marker of shell ids (`sh_…`, the command a `shell` tool call runs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellKind {}

impl IdKind for SessionKind {
    const PREFIX: &'static str = "ses_";
    const LABEL: &'static str = "session";
}

impl IdKind for MessageKind {
    const PREFIX: &'static str = "msg_";
    const LABEL: &'static str = "message";
}

impl IdKind for PermissionKind {
    const PREFIX: &'static str = "per_";
    const LABEL: &'static str = "permission";
}

impl IdKind for FormKind {
    const PREFIX: &'static str = "frm_";
    const LABEL: &'static str = "form";
}

impl IdKind for ShellKind {
    const PREFIX: &'static str = "sh_";
    const LABEL: &'static str = "shell";
}

/// An OpenCode 2 session id.
pub type SessionId = Id<SessionKind>;

/// An OpenCode 2 message id (also the id of an inbox item).
pub type MessageId = Id<MessageKind>;

/// An OpenCode 2 permission request id.
pub type PermissionId = Id<PermissionKind>;

/// An OpenCode 2 form id.
pub type FormId = Id<FormKind>;

/// An OpenCode 2 shell id.
pub type ShellId = Id<ShellKind>;

/// A checked OpenCode 2 id of kind `K` (see the module docs). Serializes as the plain string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id<K> {
    value: String,
    kind: PhantomData<K>,
}

impl<K: IdKind> Id<K> {
    /// Checks `value`: the kind's prefix, a non-empty rest of `[A-Za-z0-9]`, at most
    /// [`MAX_ID_LEN`] bytes.
    pub fn parse(value: &str) -> Result<Self, RouteError> {
        let rest = value.strip_prefix(K::PREFIX).unwrap_or_default();

        let valid =
            !rest.is_empty() && value.len() <= MAX_ID_LEN && rest.bytes().all(|byte| byte.is_ascii_alphanumeric());

        if !valid {
            return Err(RouteError::InvalidId { kind: K::LABEL });
        }

        Ok(Self {
            value: value.to_owned(),
            kind: PhantomData,
        })
    }

    /// The id as OpenCode writes it.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl<K> fmt::Display for Id<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

impl<K> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}
