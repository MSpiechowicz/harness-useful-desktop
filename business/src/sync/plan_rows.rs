//! The rows of an import plan: what the webview sees ([`SyncImportItem`]) and what Rust keeps to
//! apply it ([`StoredPlan`]). See [`super::plan`].

use serde_json::{Map, Value};

use super::collect::package_identity;
use super::plan_presence::McpFiles;
use super::schema::{Marketplace, McpServer, Package, Plugin, Transport};
use crate::ecosystem::Shape;
use crate::harness::Harness;
use crate::model::{
    ExtensionAction, HarnessId, InstallScope, SyncImportItem, SyncInvalidReason, SyncItemKind, SyncItemStatus,
    SyncMcpPreview, SyncNote, SyncStep,
};

/// The steps of a reviewed import, kept by Rust under [`StoredPlan::id`] and consumed by
/// [`super::apply_plan`]. Built only from validated bundle fields; it never holds anything read from
/// this machine besides the MCP file paths.
#[derive(Debug, Clone)]
pub struct StoredPlan {
    id: String,
    items: Vec<StoredItem>,
}

impl StoredPlan {
    pub(super) fn new(id: String, items: Vec<StoredItem>) -> Self {
        Self { id, items }
    }

    /// The plan id the webview passes back (`SyncImportPlan::plan_id`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The ids of the items that may be applied (`new` items), in plan order.
    pub fn selectable_ids(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|item| item.step.is_some())
            .map(|item| item.id.as_str())
            .collect()
    }

    /// The harnesses with at least one selectable item, in plan order: the Tauri layer resolves
    /// their binaries before applying.
    pub fn harnesses(&self) -> Vec<HarnessId> {
        let mut harnesses: Vec<HarnessId> = Vec::new();

        for item in self.items.iter().filter(|item| item.step.is_some()) {
            if !harnesses.contains(&item.harness) {
                harnesses.push(item.harness);
            }
        }

        harnesses
    }

    pub(super) fn items(&self) -> &[StoredItem] {
        &self.items
    }
}

/// One item of a [`StoredPlan`].
#[derive(Debug, Clone)]
pub(crate) struct StoredItem {
    pub id: String,
    pub harness: HarnessId,
    pub kind: SyncItemKind,
    pub label: String,
    /// What applying does; `None` when the item is not selectable.
    pub step: Option<Step>,
    /// The display line of a CLI step (program name first).
    pub command: Vec<String>,
    /// The bundle marketplace a plugin needs (see `dependsOn`).
    pub depends_on: Option<Dependency>,
    /// The MCP file of an MCP step.
    pub mcp_files: Option<McpFiles>,
}

/// A plugin's marketplace item of the same bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dependency {
    /// Its item id.
    pub id: String,
    /// Its name.
    pub marketplace: String,
}

/// What applying an item does, from validated fields only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// `MarketplaceAdd { source }`.
    Marketplace(Marketplace),
    /// `PluginInstall { target, scope: User }` (see [`Step::action`]).
    Plugin(Plugin),
    /// `PackageInstall { source }` (no project).
    Package(Package),
    /// [`mcp_definition`] merged into the harness's user MCP file.
    Mcp(McpServer),
}

impl Step {
    /// The CLI action of a marketplace, plugin or package of `harness`; `None` for an MCP server. A
    /// plugin installs its id; an OpenCode plugin installs `<id>@<version>` when it has a version
    /// (the pin is kept, not informational).
    pub fn action(&self, harness: HarnessId) -> Option<ExtensionAction> {
        match self {
            Self::Marketplace(marketplace) => Some(ExtensionAction::MarketplaceAdd {
                source: marketplace.source.clone(),
            }),
            Self::Plugin(plugin) => Some(ExtensionAction::PluginInstall {
                target: plugin_target(harness, plugin),
                scope: InstallScope::User,
            }),
            Self::Package(package) => Some(ExtensionAction::PackageInstall {
                source: package.source.clone(),
            }),
            Self::Mcp(_) => None,
        }
    }

    /// The id key: marketplace name, plugin id (without version), [`package_identity`] or server
    /// name.
    pub fn key(&self) -> String {
        match self {
            Self::Marketplace(marketplace) => marketplace.name.clone(),
            Self::Plugin(plugin) => plugin.id.clone(),
            Self::Package(package) => package_identity(&package.source),
            Self::Mcp(server) => server.name.clone(),
        }
    }

    pub fn kind(&self) -> SyncItemKind {
        match self {
            Self::Marketplace(_) => SyncItemKind::Marketplace,
            Self::Plugin(_) => SyncItemKind::Plugin,
            Self::Package(_) => SyncItemKind::Package,
            Self::Mcp(_) => SyncItemKind::McpServer,
        }
    }
}

/// A marketplace of the bundle, as reviewed, for its plugins.
pub(super) struct SharedMarketplace {
    pub name: String,
    pub id: String,
    pub status: SyncItemStatus,
}

/// One plan row.
pub(super) struct Row {
    pub item: SyncImportItem,
    pub stored: StoredItem,
}

impl Row {
    /// A valid item with `status`, not selectable yet (see [`Row::select`]).
    pub fn valid(id: String, harness: HarnessId, step: &Step, status: SyncItemStatus) -> Self {
        let kind = step.kind();
        let mut item = blank(id, kind, String::new(), status);

        match step {
            Step::Marketplace(marketplace) => {
                item.label = marketplace.name.clone();
                item.detail = Some(marketplace.source.clone());
            }
            Step::Plugin(plugin) => {
                item.label = plugin.id.clone();
                item.version = plugin.version.clone();
            }
            Step::Package(package) => {
                item.label = package.source.clone();
                item.version = package.version.clone();
            }
            Step::Mcp(server) => {
                item.label = server.name.clone();
                item.detail = Some(server.detail());
                item.runs_program = server.transport == Transport::Stdio;
                item.env_keys = server.env_keys.clone();
                item.header_keys = server.header_keys.clone();
            }
        }

        item.notes = notes(step, harness);
        let stored = stored(&item, harness);

        Self { item, stored }
    }

    /// An invalid item, shown by its safe label (or position).
    pub fn invalid(
        id: String,
        harness: HarnessId,
        kind: SyncItemKind,
        label: String,
        reason: SyncInvalidReason,
    ) -> Self {
        let item = blank(id, kind, label, SyncItemStatus::Invalid { reason });
        let stored = stored(&item, harness);

        Self { item, stored }
    }

    /// Makes a `new` row selectable with `step` and its displayed `display` step.
    pub fn select(&mut self, step: Step, display: SyncStep, depends_on: Option<Dependency>, files: Option<McpFiles>) {
        let stdio = matches!(&step, Step::Mcp(server) if server.transport == Transport::Stdio);
        let disabled = matches!(&display, SyncStep::McpMerge { server, .. } if !server.enabled);

        if disabled {
            self.item.notes.push(SyncNote::WrittenDisabled);
        }

        if depends_on.is_some() {
            self.item.notes.push(SyncNote::NeedsSharedMarketplace);
        }

        if let SyncStep::Cli { command } = &display {
            self.stored.command = command.clone();
        }

        self.item.selectable = true;
        self.item.selected_by_default = !stdio;
        self.item.depends_on = depends_on.as_ref().map(|dependency| dependency.id.clone());
        self.item.steps = vec![display];
        self.stored.step = Some(step);
        self.stored.depends_on = depends_on;
        self.stored.mcp_files = files;
    }
}

fn blank(id: String, kind: SyncItemKind, label: String, status: SyncItemStatus) -> SyncImportItem {
    SyncImportItem {
        id,
        kind,
        label,
        detail: None,
        version: None,
        status,
        selectable: false,
        selected_by_default: false,
        steps: Vec::new(),
        runs_program: false,
        env_keys: Vec::new(),
        header_keys: Vec::new(),
        notes: Vec::new(),
        depends_on: None,
    }
}

fn stored(item: &SyncImportItem, harness: HarnessId) -> StoredItem {
    StoredItem {
        id: item.id.clone(),
        harness,
        kind: item.kind,
        label: item.label.clone(),
        step: None,
        command: Vec::new(),
        depends_on: None,
        mcp_files: None,
    }
}

/// What `plugin` of `harness` installs: its id, or `<id>@<version>` for a pinned OpenCode plugin.
fn plugin_target(harness: HarnessId, plugin: &Plugin) -> String {
    match (&plugin.version, harness) {
        (Some(version), HarnessId::OpenCode) => format!("{}@{version}", plugin.id),
        _ => plugin.id.clone(),
    }
}

/// The notes of a valid item. An OpenCode plugin's version is installed as is, so it is not
/// informational there.
fn notes(step: &Step, harness: HarnessId) -> Vec<SyncNote> {
    let (version, env, headers) = match step {
        Step::Marketplace(_) => (false, false, false),
        Step::Plugin(plugin) => (plugin.version.is_some() && harness != HarnessId::OpenCode, false, false),
        Step::Package(package) => (package.version.is_some(), false, false),
        Step::Mcp(server) => (false, !server.env_keys.is_empty(), !server.header_keys.is_empty()),
    };

    [
        (version, SyncNote::VersionInformational),
        (env, SyncNote::EnvValuesNotShared),
        (headers, SyncNote::HeaderValuesNotShared),
    ]
    .into_iter()
    .filter_map(|(applies, note)| applies.then_some(note))
    .collect()
}

/// The command line of `action` as `harness` runs it (program name first, no project), or `None`
/// when the adapter refuses it.
pub(super) fn display_argv(harness: &dyn Harness, action: &ExtensionAction) -> Option<SyncStep> {
    let invocation = harness.cli_action(action, None).ok()?;
    let mut command = vec![harness.binary_name().to_owned()];
    command.extend(invocation.args);

    Some(SyncStep::Cli { command })
}

/// The MCP step shown for `server`, written to `file` (a home-relative display path).
pub(super) fn mcp_preview(file: String, server: &McpServer) -> SyncStep {
    SyncStep::McpMerge {
        file,
        server: SyncMcpPreview {
            name: server.name.clone(),
            transport: server.transport.into(),
            command: server.command.clone(),
            args: server.args.clone(),
            url: server.url.clone(),
            enabled: !needs_values(server),
        },
    }
}

/// True when the server needs env or header values the user has to fill in.
fn needs_values(server: &McpServer) -> bool {
    !server.env_keys.is_empty() || !server.header_keys.is_empty()
}

/// The entry an import writes for `server` into a file of `shape`: stdio `{command, args}`,
/// http/sse `{type, url}`, plus `"enabled": false` when it needs env or header values. OpenCode:
/// `{type: "local", command: [command, ...args], enabled}` or `{type: "remote", url, enabled}`, with
/// `enabled` false when it needs values. Nothing else, ever: env and header names are not written
/// (the review lists them), so no empty value can reach the server.
pub(crate) fn mcp_definition(server: &McpServer, shape: Shape) -> Value {
    if shape == Shape::OpenCode {
        return opencode_definition(server);
    }

    let mut entry = Map::new();

    match server.transport {
        Transport::Stdio => {
            entry.insert("command".to_owned(), server.command.clone().unwrap_or_default().into());
            entry.insert("args".to_owned(), server.args.clone().into());
        }
        Transport::Http | Transport::Sse => {
            let kind = if server.transport == Transport::Http {
                "http"
            } else {
                "sse"
            };
            entry.insert("type".to_owned(), kind.into());
            entry.insert("url".to_owned(), server.url.clone().unwrap_or_default().into());
        }
    }

    if needs_values(server) {
        entry.insert("enabled".to_owned(), false.into());
    }

    Value::Object(entry)
}

/// The OpenCode entry for `server` (see [`mcp_definition`]).
fn opencode_definition(server: &McpServer) -> Value {
    let mut entry = Map::new();

    match server.transport {
        Transport::Stdio => {
            let mut command = vec![server.command.clone().unwrap_or_default()];
            command.extend(server.args.iter().cloned());

            entry.insert("type".to_owned(), "local".into());
            entry.insert("command".to_owned(), command.into());
        }
        Transport::Http | Transport::Sse => {
            entry.insert("type".to_owned(), "remote".into());
            entry.insert("url".to_owned(), server.url.clone().unwrap_or_default().into());
        }
    }

    entry.insert("enabled".to_owned(), (!needs_values(server)).into());

    Value::Object(entry)
}
