//! What a kind's view shows of its team documents while they are shared ([`TeamSettings`]), and
//! [`team_settings!`] that implements it for the usual settings type.

/// Implements [`TeamSettings`] for a settings type with the fields `trust`, `problem` and
/// `refreshed_at` (`TeamHooks`, `TeamRoutines`).
macro_rules! team_settings {
    ($settings:ty) => {
        impl $crate::team_sync::TeamSettings for $settings {
            fn shared(trust: bool) -> Self {
                Self {
                    trust,
                    problem: None,
                    refreshed_at: None,
                }
            }

            fn trust(&mut self) -> &mut bool {
                &mut self.trust
            }

            fn problem(&mut self) -> &mut Option<String> {
                &mut self.problem
            }

            fn refreshed_at(&mut self) -> &mut Option<String> {
                &mut self.refreshed_at
            }
        }
    };
}

pub(crate) use team_settings;

/// What the kind's view shows of its team documents while they are shared (`TeamHooks`).
pub trait TeamSettings: Clone + PartialEq + Send + Sync + 'static {
    /// The settings when sharing is found on, trusted as `trust` says, before any refresh.
    fn shared(trust: bool) -> Self;

    /// "Trust team …" is on: team documents are used without local approval.
    fn trust(&mut self) -> &mut bool;

    /// Why the latest refresh failed, `None` after a good one.
    fn problem(&mut self) -> &mut Option<String>;

    /// When the copies were last refreshed (RFC 3339 UTC).
    fn refreshed_at(&mut self) -> &mut Option<String>;
}
