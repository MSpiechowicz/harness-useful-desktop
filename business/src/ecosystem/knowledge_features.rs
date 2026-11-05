//! What the installed Harness Knowledge can do: `harness-knowledge features` prints one feature
//! per line and exits 0. The desktop reads these and offers nothing without its feature:
//!
//! - `docs`: `docs list|get|links|rename|delete`, `repos`, `stats` and `check --json`;
//! - `import-ndjson`: `docs import --ndjson`, the streamed import;
//! - `embedder`: its own local embedder (`embedder run|enable|disable|status|remove-model`) behind
//!   vector search;
//! - `search`: `search --json`;
//! - `recall`: `recall --hook`, the pointers added to a prompt;
//! - `held`: the review queue of proposed documents (`held list|get|approve|reject`);
//! - `mcp`: the MCP server the harnesses run;
//! - `routing`: `[routing]` in the config, ids `local:N` and the `place` of documents;
//! - `doc-matches`: `docs matches`;
//! - `offline-fallback`: answers from this computer while the remote database cannot be reached;
//! - `docs-useful`: `docs list|get` rows carry `shown`, `useful` and `lastUsefulAt`, and `--useful
//!   any|ever|never` and `--sort useful|lastUseful` use them;
//! - `docs-similar`: `docs similar`, the manifest's `replacing` and the check of `knowledge_add`;
//! - `docs-authors`: `docs list|get` rows carry `addedBy` and `updatedBy`;
//! - `docs-drafts`: document states (`live`, `draft`, `waiting`, `changesRequested`), `--status`,
//!   `--draft`, history, compare, ask and go live, reviews with comments, the stored original
//!   (download), the draft counts of `stats` and `upgrade`;
//! - `docs-approval`: team approval (`approval get|set`), approve and request changes;
//! - `docs-sort-paths`: `docs list --sort paths`.
//!
//! An older server does not know the command and exits 2. [`KnowledgeFeatures::support`] asks once
//! per binary through a [`LineProbe`] (cached per binary, an older server cached as one without
//! features, a failed probe not cached).

use std::path::PathBuf;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::sidecar_embedder::sidecar_feature_server;
use super::sidecar_line_probe::{LineAnswer, LineProbe};
use crate::env::Env;
use crate::error::Result;
use crate::model::{KnowledgeAvailability, KnowledgeFeatures as FeatureSet, KnowledgeStatus};
use crate::process::ProcessRunner;

/// The line for the document commands.
const DOCS: &str = "docs";

/// The line for the streamed import.
const IMPORT_NDJSON: &str = "import-ndjson";

/// The line for the local embedder.
const EMBEDDER: &str = "embedder";

/// The line for `search --json`.
const SEARCH: &str = "search";

/// The line for `recall --hook`.
const RECALL: &str = "recall";

/// The line for the review queue.
const HELD: &str = "held";

/// The line for the MCP server.
const MCP: &str = "mcp";

/// The line for routing documents between the remote database and this computer.
const ROUTING: &str = "routing";

/// The line for `docs matches`.
const DOC_MATCHES: &str = "doc-matches";

/// The line for answering from this computer while the remote database cannot be reached.
const OFFLINE_FALLBACK: &str = "offline-fallback";

/// The line for the usefulness counts of a document, `--useful` and the sorts by them.
const DOCS_USEFUL: &str = "docs-useful";

/// The line for `docs similar` and replacing a document found by it.
const DOCS_SIMILAR: &str = "docs-similar";

/// The line for the authors of a document.
const DOCS_AUTHORS: &str = "docs-authors";

/// The line for drafts, their review and `upgrade`.
const DOCS_DRAFTS: &str = "docs-drafts";

/// The line for the sort by how many paths a document is narrowed to.
const DOCS_SORT_PATHS: &str = "docs-sort-paths";

/// The line for team approval.
const DOCS_APPROVAL: &str = "docs-approval";

/// What the installed server says it can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeSupport {
    /// It answers `features`: which of the lines the desktop reads it listed.
    Listed(FeatureSet),
    /// It predates `features` (or failed it): none of them.
    Unsupported,
    /// Harness Knowledge is not installed.
    NotInstalled,
    /// The server could not be asked (it did not start or answer in time).
    Unknown,
}

impl KnowledgeSupport {
    /// The features listed, none while the server could not be asked.
    pub fn features(self) -> FeatureSet {
        match self {
            Self::Listed(features) => features,
            Self::Unsupported | Self::NotInstalled | Self::Unknown => FeatureSet::default(),
        }
    }

    /// The status the UI shows: `available` once the server listed its features.
    pub fn status(self) -> KnowledgeStatus {
        let availability = match self {
            Self::Listed(_) => KnowledgeAvailability::Available,
            Self::Unsupported => KnowledgeAvailability::Outdated,
            Self::NotInstalled => KnowledgeAvailability::NotInstalled,
            Self::Unknown => KnowledgeAvailability::Failed {
                message: "Harness Knowledge did not answer".to_owned(),
            },
        };

        KnowledgeStatus {
            availability,
            features: self.features(),
        }
    }

    /// Whether the server has `docs list|get|links|rename|delete`: `docs` is listed.
    pub fn docs(self) -> bool {
        self.features().docs
    }

    /// Whether the server has `docs import --ndjson`: `import-ndjson` is listed.
    pub fn import(self) -> bool {
        self.features().import
    }

    /// Whether the server runs a local embedder: `embedder` is listed.
    pub fn embedder(self) -> bool {
        self.features().embedder
    }

    /// Whether the server has `search --json`: `search` is listed.
    pub fn search(self) -> bool {
        self.features().search
    }

    /// Whether the server has `recall --hook`: `recall` is listed.
    pub fn recall(self) -> bool {
        self.features().recall
    }

    /// Whether the server has the review queue: `held` is listed.
    pub fn held(self) -> bool {
        self.features().held
    }

    /// Whether the server routes documents: `routing` is listed.
    pub fn routing(self) -> bool {
        self.features().routing
    }

    /// Whether the server has `docs matches`: `doc-matches` is listed.
    pub fn doc_matches(self) -> bool {
        self.features().doc_matches
    }

    /// Whether the server answers from this computer while the remote database cannot be reached:
    /// `offline-fallback` is listed.
    pub fn offline_fallback(self) -> bool {
        self.features().offline_fallback
    }

    /// Whether documents carry usefulness counts and `docs list` filters and sorts by them:
    /// `docs-useful` is listed.
    pub fn docs_useful(self) -> bool {
        self.features().docs_useful
    }

    /// Whether the server has `docs similar` and the import's `replacing`: `docs-similar` is
    /// listed.
    pub fn docs_similar(self) -> bool {
        self.features().docs_similar
    }

    /// Whether documents carry the authors who added and changed them: `docs-authors` is listed.
    pub fn docs_authors(self) -> bool {
        self.features().docs_authors
    }

    /// Whether the server keeps drafts and their review: `docs-drafts` is listed.
    pub fn docs_drafts(self) -> bool {
        self.features().docs_drafts
    }

    /// Whether the server has team approval: `docs-approval` is listed.
    pub fn docs_approval(self) -> bool {
        self.features().docs_approval
    }
}

/// The probe of the installed server's features. See the module docs.
#[derive(Debug)]
pub struct KnowledgeFeatures {
    probe: LineProbe,
}

impl Default for KnowledgeFeatures {
    fn default() -> Self {
        Self {
            probe: LineProbe::new(&KNOWLEDGE_SIDECAR, "features"),
        }
    }
}

impl KnowledgeFeatures {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the installed server can do (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> KnowledgeSupport {
        match self.probe.ask(env, runner).await {
            answer @ LineAnswer::Listed(_) => KnowledgeSupport::Listed(FeatureSet {
                docs: answer.lists(DOCS),
                import: answer.lists(IMPORT_NDJSON),
                embedder: answer.lists(EMBEDDER),
                search: answer.lists(SEARCH),
                recall: answer.lists(RECALL),
                held: answer.lists(HELD),
                mcp: answer.lists(MCP),
                routing: answer.lists(ROUTING),
                doc_matches: answer.lists(DOC_MATCHES),
                offline_fallback: answer.lists(OFFLINE_FALLBACK),
                docs_useful: answer.lists(DOCS_USEFUL),
                docs_similar: answer.lists(DOCS_SIMILAR),
                docs_authors: answer.lists(DOCS_AUTHORS),
                docs_drafts: answer.lists(DOCS_DRAFTS),
                docs_approval: answer.lists(DOCS_APPROVAL),
                docs_sort_paths: answer.lists(DOCS_SORT_PATHS),
            }),
            LineAnswer::Unsupported => KnowledgeSupport::Unsupported,
            LineAnswer::NotInstalled => KnowledgeSupport::NotInstalled,
            LineAnswer::Unknown => KnowledgeSupport::Unknown,
        }
    }

    /// The installed server when `feature` of its `features` is listed: `notFound` when Harness
    /// Knowledge is not installed, `unsupported` (naming `what`) when it predates the feature.
    pub async fn feature_server(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        feature: fn(KnowledgeSupport) -> bool,
        what: &str,
    ) -> Result<PathBuf> {
        let listed = async { feature(self.support(env, runner).await) };

        sidecar_feature_server(&KNOWLEDGE_SIDECAR, env, listed, what).await
    }
}

impl super::sidecar_embedder_supervisor::EmbedderFeatures for KnowledgeFeatures {
    async fn embedder_server(&self, env: &Env, runner: &dyn ProcessRunner) -> Result<PathBuf> {
        self.feature_server(env, runner, KnowledgeSupport::embedder, "vector search")
            .await
    }
}
