//! Whether Useful Skills are installed for a chat's harness: the lookup the Harness Skills hook
//! step waits for in that chat.

use std::time::Instant;

use harness_business::ecosystem::useful_skills_installed_for;
use harness_business::harness::HarnessRegistry;
use harness_business::model::HarnessId;

use super::{HookRuntime, MEMBERS_TTL, seed_available};

impl HookRuntime {
    /// Whether Useful Skills are installed for `harness`: what a chat of that harness gets
    /// ([`harness_business::hooks::ChatContext::useful_skills`]). Read from the plugin registries,
    /// so kept for [`MEMBERS_TTL`] per harness.
    ///
    /// The first lookup since launch that finds them also seeds the family hooks that were never
    /// seeded (the Harness Skills hook after the skills were installed through a harness's own
    /// plugin or package action, which the app does not see): never one the user deleted.
    pub async fn useful_skills_for(&self, harness: HarnessId) -> bool {
        let (cached, generation) = {
            let cache = self.lock_cache();

            let fresh = cache
                .skills
                .get(&harness)
                .filter(|(at, _)| at.elapsed() < MEMBERS_TTL)
                .map(|(_, installed)| *installed);

            (fresh, cache.generation)
        };

        if let Some(installed) = cached {
            return installed;
        }

        let env = self.env.clone();

        let installed = tauri::async_runtime::spawn_blocking(move || {
            useful_skills_installed_for(&env, &HarnessRegistry::with_defaults(), harness)
        })
        .await
        .unwrap_or(false);

        if installed && self.first_skills_found() {
            self.refresh_members();
            seed_available(self).await;
        }

        // Not kept when something changed meanwhile (an install, a trust change): that read may
        // predate it.
        let mut cache = self.lock_cache();

        if cache.generation == generation {
            cache.skills.insert(harness, (Instant::now(), installed));
        }

        installed
    }

    /// True once per launch.
    fn first_skills_found(&self) -> bool {
        !std::mem::replace(&mut self.lock_cache().skills_seeded, true)
    }
}
