//! Whether the Useful Skills are installed, per harness, for the Harness Skills step of the hooks
//! (see [`crate::hooks`]): the same check as the "Recommended setup" step's `skills` state
//! ([`super::setup`]).
//!
//! It reads plugin registries and install records, so call it from a blocking-capable context and
//! keep its answer (it does not change within a chat).

use super::setup::skills_setup;
use crate::env::Env;
use crate::harness::{Harness, HarnessRegistry};
use crate::model::{HarnessId, SkillsSetup};

/// True when the Useful Skills are installed for `harness`.
pub fn useful_skills_installed(env: &Env, harness: &dyn Harness) -> bool {
    matches!(skills_setup(env, harness), SkillsSetup::Installed { .. })
}

/// True when the Useful Skills are installed for the harness `id` of `registry`.
pub fn useful_skills_installed_for(env: &Env, registry: &HarnessRegistry, id: HarnessId) -> bool {
    registry
        .get(id)
        .is_some_and(|harness| useful_skills_installed(env, harness))
}

/// True when the Useful Skills are installed for at least one harness of `registry`.
pub fn useful_skills_installed_anywhere(env: &Env, registry: &HarnessRegistry) -> bool {
    registry.all().any(|harness| useful_skills_installed(env, harness))
}
