//! The check a team run makes at its start: a team routine that runs on trust alone (its exact
//! bytes not approved on this machine) runs only while the team database still holds the version
//! this machine copied. The copy in `<root>/team/` is a file any process of the user can write, so
//! trust covers what the team database holds, not what the copy says.

use std::path::Path;

use crate::ecosystem::memory_team_docs::get_team_doc;
use crate::env::Env;
use crate::error::BusinessError;
use crate::model::{HookApproval, HookOrigin, RoutineView};
use crate::process::ProcessRunner;
use crate::routines::RoutineKind;
use crate::signed_docs::sha256_hex;

/// For the routine of `view` about to run: `Ok` for a local routine and for a team routine whose
/// exact bytes are approved here, else Harness Memory (`server`, `routines get <id> --json`) must
/// still hold a body with the copy's SHA-256 (`view.sha256`). Otherwise why the run cannot go on.
pub async fn confirm_team_copy(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    view: &RoutineView,
) -> std::result::Result<(), &'static str> {
    if view.origin != HookOrigin::Team || view.approval == HookApproval::Approved {
        return Ok(());
    }

    match get_team_doc::<RoutineKind>(env, runner, server, &view.id).await {
        Ok(row) if row.sha256 == view.sha256 && sha256_hex(row.body.as_bytes()) == view.sha256 => Ok(()),
        Ok(_) => Err("The remote routine changed in the remote database since it was copied here"),
        Err(BusinessError::NotFound(_)) => Err("The remote routine is no longer in the remote database"),
        Err(error) => {
            tracing::warn!(code = ?error.code(), "team routine not confirmed");
            Err("Harness Memory could not confirm the remote routine")
        }
    }
}
