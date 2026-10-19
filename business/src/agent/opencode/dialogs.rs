//! Dialogs of an [`OpenCodeSession`]: OpenCode's permission asks and questions. The chat does not
//! show them yet, so no dialog is ever open to answer.

use super::OpenCodeSession;
use crate::error::{BusinessError, Result};
use crate::model::UiResponse;

impl OpenCodeSession {
    /// Answers an open dialog; none can be open yet.
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;

        Err(BusinessError::Unsupported(format!(
            "dialog {} cannot be answered: OpenCode approvals and questions are not available yet",
            response.id
        )))
    }
}
