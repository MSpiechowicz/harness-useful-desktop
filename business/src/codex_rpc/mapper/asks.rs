//! What Codex asks the user, as data for the engine's dialogs, answered with the encoders of
//! [`answers`](crate::codex_rpc::wire::answers) under [`Ask::request_id`]:
//!
//! - a command: [`command_decision`](crate::codex_rpc::wire::answers::command_decision) (or
//!   `command_amendment` for its proposed rule, when offered);
//! - a file change, with its files from the item's `item/started` (Codex names only the item):
//!   `file_change_decision`;
//! - extra permissions: `permissions_grant` or `permissions_deny`;
//! - the model's questions: `user_input_answers`;
//! - an MCP server's form or URL: `elicitation_answer`.
//!
//! Commands and paths are kept as sent: whatever shows them must redact them.

use super::tools::Tools;
use crate::codex_rpc::wire::RequestId;
use crate::codex_rpc::wire::items::FileUpdateChange;
use crate::codex_rpc::wire::requests::{
    CommandApproval, Elicitation, FileChangeApproval, PermissionsApproval, ServerRequest, ServerRequestKind,
    UserInputRequest,
};

/// Something Codex waits for the user to answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// Answer under this id; `serverRequest/resolved` names it when the ask is over.
    pub request_id: RequestId,
    pub kind: AskKind,
}

/// What an [`Ask`] is about.
#[derive(Debug, Clone, PartialEq)]
pub enum AskKind {
    /// May the command run?
    Command(Box<CommandApproval>),
    /// May the patch be applied to `changes`?
    FileChange {
        approval: FileChangeApproval,
        /// The files and patches of the item (empty when its start was not seen).
        changes: Vec<FileUpdateChange>,
    },
    /// May the turn or session get extra permissions?
    Permissions(Box<PermissionsApproval>),
    /// The model's questions.
    Questions(UserInputRequest),
    /// An MCP server asks for a form's values or to visit a URL.
    Elicitation(Box<Elicitation>),
}

impl Ask {
    /// The tool card the ask is about, when it is about one.
    pub fn item_id(&self) -> Option<&str> {
        let id = match &self.kind {
            AskKind::Command(approval) => &approval.item_id,
            AskKind::FileChange { approval, .. } => &approval.item_id,
            AskKind::Permissions(approval) => &approval.item_id,
            AskKind::Questions(request) => &request.item_id,
            AskKind::Elicitation(_) => return None,
        };

        (!id.is_empty()).then_some(id.as_str())
    }
}

/// What `request` asks the user; `None` for a request the app refuses.
pub(super) fn ask(request: &ServerRequest, tools: &Tools) -> Option<Ask> {
    let kind = match &request.kind {
        ServerRequestKind::CommandApproval(approval) => AskKind::Command(approval.clone()),
        ServerRequestKind::FileChangeApproval(approval) => AskKind::FileChange {
            approval: approval.clone(),
            changes: tools.file_changes(&approval.item_id).to_vec(),
        },
        ServerRequestKind::PermissionsApproval(approval) => AskKind::Permissions(approval.clone()),
        ServerRequestKind::UserInput(request) => AskKind::Questions(request.clone()),
        ServerRequestKind::Elicitation(elicitation) => AskKind::Elicitation(elicitation.clone()),
        ServerRequestKind::DynamicToolCall | ServerRequestKind::AuthTokensRefresh | ServerRequestKind::Unknown => {
            return None;
        }
    };

    Some(Ask {
        request_id: request.id.clone(),
        kind,
    })
}
