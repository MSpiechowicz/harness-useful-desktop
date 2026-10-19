//! Why the installed server cannot take a choice of [`memory_remote_set`](super::memory_remote_set)
//! or [`memory_remote_test`](super::memory_remote_test): Amazon DocumentDB, and notes or scripts
//! routed `remote+local`. Each answers `None` when it can.

use crate::ecosystem::memory_features::FeaturesSupport;
use crate::ecosystem::memory_kinds::KindsSupport;

/// Why Amazon DocumentDB cannot be used with the installed server, if it cannot.
pub(super) fn documentdb_unavailable(kinds: KindsSupport) -> Option<String> {
    let reason = match kinds {
        KindsSupport::Listed { documentdb: true } => return None,
        KindsSupport::Listed { documentdb: false } | KindsSupport::Unsupported => {
            "Update Harness Memory to use Amazon DocumentDB"
        }
        KindsSupport::NotInstalled => "Install Harness Memory to use Amazon DocumentDB",
        KindsSupport::Unknown => "Harness Memory could not say whether it can use Amazon DocumentDB. Try again.",
    };

    Some(reason.to_owned())
}

/// Why a note scope cannot be routed `remote+local` with the installed server, if it cannot.
pub(super) fn team_and_local_unavailable(features: FeaturesSupport) -> Option<String> {
    let reason = match features {
        _ if features.team_and_local() => return None,
        FeaturesSupport::Listed { .. } | FeaturesSupport::Unsupported => {
            "Update Harness Memory to read remote notes and save new ones on this computer"
        }
        FeaturesSupport::NotInstalled => {
            "Install Harness Memory to read remote notes and save new ones on this computer"
        }
        FeaturesSupport::Unknown => {
            "Harness Memory could not say whether it can read remote notes and save new ones on this computer. Try again."
        }
    };

    Some(reason.to_owned())
}

/// Why scripts cannot be routed `remote+local` with the installed server, if they cannot.
pub(super) fn scripts_team_and_local_unavailable(features: FeaturesSupport) -> Option<String> {
    let reason = match features {
        _ if features.scripts_team_and_local() => return None,
        FeaturesSupport::Listed { .. } | FeaturesSupport::Unsupported => {
            "Update Harness Memory to read remote scripts and save new ones on this computer"
        }
        FeaturesSupport::NotInstalled => {
            "Install Harness Memory to read remote scripts and save new ones on this computer"
        }
        FeaturesSupport::Unknown => {
            "Harness Memory could not say whether it can read remote scripts and save new ones on this computer. Try again."
        }
    };

    Some(reason.to_owned())
}
