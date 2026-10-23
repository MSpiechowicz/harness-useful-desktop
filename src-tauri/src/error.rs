//! The error type every IPC command returns.

use harness_business::BusinessError;
use harness_business::model::{IpcError, IpcErrorCode};
use serde::Serialize;

/// Result of an IPC command.
pub type CommandResult<T> = Result<T, CommandError>;

/// A failed IPC command. Serializes exactly as the C0 [`IpcError`]: `{ "code", "message" }`.
///
/// Messages never carry secret values or file contents (a [`BusinessError`] rule). `Forbidden`
/// messages are replaced by a generic text because they may name a deny-listed file or a path
/// outside an allowed root; the UI branches on `code` only.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct CommandError(IpcError);

/// Shown instead of the business message for every `Forbidden` error.
const FORBIDDEN_MESSAGE: &str = "access refused by a security rule";

impl CommandError {
    /// A bug or unexpected state in the shell itself.
    pub fn internal(message: impl Into<String>) -> Self {
        Self(IpcError {
            code: IpcErrorCode::Internal,
            message: message.into(),
        })
    }

    /// The message, which carries no secret values or file contents.
    pub fn message(&self) -> &str {
        &self.0.message
    }

    /// The error category (what the UI branches on).
    pub fn code(&self) -> IpcErrorCode {
        self.0.code
    }
}

impl From<BusinessError> for CommandError {
    fn from(error: BusinessError) -> Self {
        let mut payload = IpcError::from(error);

        if payload.code == IpcErrorCode::Forbidden {
            payload.message = FORBIDDEN_MESSAGE.to_owned();
        }

        Self(payload)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn serializes_as_ipc_error() {
        let error = CommandError::from(BusinessError::InvalidInput("bad theme".to_owned()));

        let value = serde_json::to_value(&error).expect("serializable");

        assert_eq!(
            value,
            json!({ "code": "invalidInput", "message": "invalid input: bad theme" })
        );
    }

    #[test]
    fn forbidden_errors_hide_the_path() {
        let error = CommandError::from(BusinessError::Forbidden(
            "/home/user/.omp/agent/auth.json: secret".to_owned(),
        ));

        let value = serde_json::to_value(&error).expect("serializable");

        assert_eq!(value, json!({ "code": "forbidden", "message": FORBIDDEN_MESSAGE }));
    }
}
