//! Notices changes to the hooks folder and the approvals ([`crate::doc_watch`] for hooks). A
//! change reads the hooks again on their next firing and sends `changed` to the Hooks view.

use std::sync::Arc;

use harness_business::hooks::HookKind;

use super::HookRuntime;
use crate::doc_watch;

/// Starts the watcher thread (it runs until the app exits). Does nothing without a hooks folder.
pub fn spawn(runtime: Arc<HookRuntime>) {
    if runtime.store.is_none() {
        return;
    }

    doc_watch::spawn::<HookKind>(move || {
        runtime.poll();
    });
}

impl HookRuntime {
    /// One look at the folder and the approvals (see [`doc_watch::DocWatch::poll`]). The first
    /// look only takes the fingerprint and drops what is left of removed hooks. True when
    /// something changed.
    pub fn poll(&self) -> bool {
        let Some(store) = &self.store else {
            return false;
        };

        if !self.watch.poll(store, &self.writes) {
            return false;
        }

        self.invalidate();
        self.notify();
        true
    }
}
