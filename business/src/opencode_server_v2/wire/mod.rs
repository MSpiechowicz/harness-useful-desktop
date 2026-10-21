//! The OpenCode 2 server's JSON, read field by field (recordings in
//! `tests/fixtures/opencode2/recorded`). Pure: no I/O.
//!
//! - [`envelope`]: the `{data}` response envelope, list cursors, `/api/info` and `interrupt`.
//! - [`session`]: the session object, model reference, token counts and session request bodies.
//! - [`catalog`]: the agent, command, model and provider lists and the configuration sources.
//! - [`prompt`]: the prompt, command, shell and compact bodies, inbox items, permission requests,
//!   forms and their reply bodies.

pub mod catalog;
pub mod envelope;
pub mod prompt;
pub mod session;

pub use catalog::{Agent, Capabilities, Command, ConfigSource, Limit, Model, Provider, Variant};
pub use envelope::{Cursor, Envelope, Interrupted, Location, ServerInfo};
pub use prompt::{
    AgentMention, CommandBody, CompactBody, Decision, Delivery, FileRef, Form, FormField, FormOption, FormReplyBody,
    FormValue, InboxEntry, InboxItem, InboxPayload, InboxTime, PermissionReplyBody, PermissionRequest,
    PermissionSource, PromptBody, ShellBody,
};
pub use session::{
    AgentBody, CacheTokens, CreateBody, ErrorInfo, ForkBody, ForkBoundary, ForkInfo, ModelBody, ModelRef, Session,
    SessionTime, TitleBody, Tokens,
};
