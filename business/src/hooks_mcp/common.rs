//! What the hook and routine tools share: the list, get and remove arguments, the project an
//! agent names, where a document applies after an agent's edit, the refusal texts, and a
//! document as the agent sees it (secrets masked).

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::Server;
use crate::error::BusinessError;
use crate::model::{HookApproval, HookAuthor, HookScope, HookView, RoutineView};
use crate::security::redact_text;
use crate::signed_docs::scope::{FileOwner, ScopedDoc};
use crate::signed_docs::{DocKind, DocStore, project_identity};

/// A tool result: the reply object (as text and as `structuredContent`), or a refusal.
pub(super) type Reply = Result<Value, String>;

/// Which documents a list tool shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum ListScope {
    Global,
    Project,
    #[default]
    All,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListArgs {
    pub(super) project: Option<String>,
    #[serde(default)]
    pub(super) scope: ListScope,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GetArgs {
    pub(super) id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RemoveArgs {
    pub(super) id: String,
    pub(super) sha256: String,
}

pub(super) fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|error| format!("invalid arguments: {error}"))
}

/// The store of kind `K`, or why there is none on this computer.
pub(super) fn store_of<K: DocKind>(store: Option<&DocStore<K>>) -> Result<&DocStore<K>, String> {
    store.ok_or_else(|| format!("{} are not available on this computer (no folder for them)", K::PLURAL))
}

/// The refusal text of a store error for a document of kind `K`, telling the agent what to do next.
pub(super) fn refusal<K: DocKind>(error: BusinessError) -> String {
    let noun = K::NOUN;

    match error {
        BusinessError::Conflict(_) => format!(
            "the {noun} changed since you read it: call {noun}_get for its current sha256, check the change, then retry"
        ),
        BusinessError::NotFound(what) if what.starts_with(&format!("the {noun}")) => format!("no {noun} has this id"),
        BusinessError::InvalidInput(problem) | BusinessError::Forbidden(problem) | BusinessError::NotFound(problem) => {
            problem
        }
        other => other.to_string(),
    }
}

/// Removes document `id` of kind `K` at version `sha256` as an agent (only one an agent created).
pub(super) fn remove<K: DocKind>(store: Option<&DocStore<K>>, args: RemoveArgs) -> Reply {
    store_of(store)?
        .delete_as(&args.id, HookAuthor::Agent, Some(&args.sha256))
        .map_err(refusal::<K>)?;

    Ok(json!({
        "removed": args.id,
        "note": format!("The {} is removed. Harness Desktop no longer runs it.", K::NOUN),
    }))
}

/// A listed file as the tools see it: hooks and routines alike.
pub(super) trait DocView {
    /// The parsed document; `None` for an invalid file.
    fn doc(&self) -> Option<&dyn ScopedDoc>;

    /// The file's fields the agent sees, the document's own fields excluded.
    fn head(&self) -> ViewHead<'_>;

    /// The document's fields as JSON; `None` for an invalid file.
    fn doc_json(&self) -> Option<Value>;
}

/// What every listed file shows besides its document.
pub(super) struct ViewHead<'a> {
    id: &'a str,
    name: Option<&'a str>,
    status: Value,
    approval: HookApproval,
    sha256: &'a str,
    problem: Option<&'a str>,
}

/// Implements [`DocView`] for a view type whose parsed document is in `$field`.
macro_rules! doc_view {
    ($view:ty, $field:ident) => {
        impl DocView for $view {
            fn doc(&self) -> Option<&dyn ScopedDoc> {
                self.$field.as_ref().map(|doc| doc as &dyn ScopedDoc)
            }

            fn head(&self) -> ViewHead<'_> {
                ViewHead {
                    id: &self.id,
                    name: self.name.as_deref(),
                    status: json!(self.status),
                    approval: self.approval,
                    sha256: &self.sha256,
                    problem: self.problem.as_deref(),
                }
            }

            fn doc_json(&self) -> Option<Value> {
                self.$field
                    .as_ref()
                    .and_then(|doc| serde_json::to_value(doc).ok())
            }
        }
    };
}

doc_view!(HookView, hook);
doc_view!(RoutineView, routine);

/// One document as the agent sees it: `id`, `name`, `status`, `approval` (`approved` or
/// `pending`), `sha256`, then the document's fields, or `problem` for an invalid file. The texts
/// of its action (hook text, prompt, command) are shown with secrets masked, the file keeps them.
pub(super) fn view_json(view: &dyn DocView) -> Value {
    let head = view.head();
    let approval = match head.approval {
        HookApproval::Approved => "approved",
        HookApproval::Changed | HookApproval::Never => "pending",
    };

    let mut object = Map::new();
    object.insert("id".to_owned(), json!(head.id));
    object.insert("name".to_owned(), json!(head.name));
    object.insert("status".to_owned(), head.status);
    object.insert("approval".to_owned(), json!(approval));
    object.insert("sha256".to_owned(), json!(head.sha256));

    if let Some(Value::Object(fields)) = view.doc_json() {
        for (key, value) in fields {
            object.entry(key).or_insert(value);
        }
    }

    if let Some(Value::Object(action)) = object.get_mut("action") {
        for field in ["text", "command", "prompt"] {
            if let Some(Value::String(text)) = action.get_mut(field) {
                *text = redact_text(text);
            }
        }
    }

    if let Some(problem) = head.problem {
        object.insert("problem".to_owned(), json!(problem));
    }

    Value::Object(object)
}

/// The document an agent edits (`doc`, parsed from `view`): only at the version the agent read
/// (`sha256`), and only when the file is valid.
pub(super) fn editable<K: DocKind, D>(view: &dyn DocView, sha256: &str, doc: Option<D>) -> Result<D, String> {
    let head = view.head();

    if head.sha256 != sha256 {
        return Err(refusal::<K>(BusinessError::Conflict(String::new())));
    }

    doc.ok_or_else(|| {
        let problem = head.problem.unwrap_or_default();
        let (noun, view_name) = (K::NOUN, title(K::PLURAL));

        format!(
            "the {noun} file is invalid ({problem}): ask the user to fix or delete it in Harness Desktop's {view_name} view"
        )
    })
}

/// The reply of a list tool for kind `K`: the files of `views` that `scope` and `project` keep,
/// and how many of each scope are used, or why the folder is not read (`folder_problem`). A file
/// that is not valid is shown with every listing (scope `all`), so it can be fixed.
pub(super) fn list_reply<'a, K: DocKind>(
    project: Option<String>,
    folder_problem: Option<String>,
    views: impl Iterator<Item = &'a dyn DocView>,
    scope: ListScope,
    note: &str,
) -> Reply {
    if let Some(problem) = folder_problem {
        return Err(format!("the {} folder is not read: {problem}", K::PLURAL));
    }

    let applies = |view: &&dyn DocView| match view.doc() {
        Some(doc) => match doc.doc_scope() {
            HookScope::Global => scope != ListScope::Project,
            HookScope::Project => scope != ListScope::Global && doc.doc_project() == project.as_deref(),
        },
        None => scope == ListScope::All,
    };

    let shown: Vec<&dyn DocView> = views.filter(applies).collect();
    let count = |wanted: HookScope| {
        let used = shown
            .iter()
            .filter(|view| view.doc().is_some_and(|doc| doc.doc_scope() == wanted))
            .count();
        format!("{used} of {}", K::MAX_PER_SCOPE)
    };

    let plural = K::PLURAL;
    let project_key = format!("project{}", title(plural));

    let mut reply = Map::new();
    reply.insert("project".to_owned(), json!(project));
    reply.insert(plural.to_owned(), shown.iter().map(|view| view_json(*view)).collect());
    reply.insert("global".to_owned(), json!(count(HookScope::Global)));
    reply.insert(project_key, json!(count(HookScope::Project)));
    reply.insert("note".to_owned(), json!(note));

    Ok(Value::Object(reply))
}

impl Server {
    /// The project a list tool shows: `project` or the working folder, turned into a project root.
    /// `None` outside a usable working folder, where only the global documents apply.
    pub(super) fn list_project(&self, args: &ListArgs) -> Result<Option<String>, String> {
        let explicit = args.project.is_some();

        match self.project_root(args.project.as_deref()) {
            Ok(root) => Ok(Some(root)),
            Err(_) if !explicit && args.scope != ListScope::Project => Ok(None),
            Err(problem) => Err(problem),
        }
    }

    /// The project root of the server's working folder, the project the agent works in; `None`
    /// when it is unknown or not a folder.
    pub(super) fn own_project(&self) -> Option<String> {
        self.project_root(None).ok()
    }

    /// `view` (a document of kind `K`) when a list of the server's own project would show it.
    /// Another project's document is not there for the agent: reading, editing and removing it
    /// all answer "no <noun> has this id".
    pub(super) fn shown<K: DocKind, V: DocView>(&self, view: V) -> Result<V, String> {
        let own = self.own_project();
        let visible = view.doc().is_none_or(|doc| FileOwner::of(doc).shown_in(own.as_deref()));

        if !visible {
            return Err(refusal::<K>(BusinessError::NotFound(format!("the {}", K::NOUN))));
        }

        Ok(view)
    }

    /// The project a document of `scope` stores after an agent's write, `None` for a global one.
    /// `project` is the folder the agent named, `current` the document it edits. An edit keeps
    /// the project root the document has unless the agent names a folder, and where a document
    /// the user created applies stays the user's choice.
    pub(super) fn placement<K: DocKind>(
        &self,
        scope: HookScope,
        project: Option<&str>,
        current: Option<&dyn ScopedDoc>,
    ) -> Result<Option<String>, String> {
        let noun = K::NOUN;

        let placed = match scope {
            HookScope::Global => {
                if project.is_some() {
                    return Err(format!(
                        "project belongs to scope project, a global {noun} applies to every project"
                    ));
                }

                None
            }
            HookScope::Project => match (project, current.and_then(|doc| doc.doc_project())) {
                (Some(folder), _) => Some(self.project_root(Some(folder))?),
                (None, Some(root)) => Some(root.to_owned()),
                (None, None) => Some(self.project_root(None)?),
            },
        };

        let Some(current) = current else {
            return Ok(placed);
        };

        let moved = scope != current.doc_scope() || placed.as_deref() != current.doc_project();

        if current.doc_created_by() == HookAuthor::Desktop && moved {
            return Err(format!(
                "this {noun} was created in Harness Desktop: its scope and project stay as they are, ask the user to \
                 change them"
            ));
        }

        Ok(placed)
    }

    /// The project root of `project` (absolute) or of the server's working folder.
    fn project_root(&self, project: Option<&str>) -> Result<String, String> {
        let folder = self.folder(project)?;
        let root = project_identity(&folder).map_err(|error| project_problem(&folder, &error))?;

        root.into_os_string()
            .into_string()
            .map_err(|_| "the project path is not valid UTF-8".to_owned())
    }

    fn folder(&self, project: Option<&str>) -> Result<PathBuf, String> {
        match project {
            Some(folder) if Path::new(folder).is_absolute() => Ok(PathBuf::from(folder)),
            Some(_) => Err("project must be an absolute folder path".to_owned()),
            None => self
                .cwd
                .clone()
                .ok_or_else(|| "the server's working folder is unknown: pass project".to_owned()),
        }
    }
}

/// `word` with its first letter upper case (`hooks` → `Hooks`).
fn title(word: &str) -> String {
    let mut chars = word.chars();

    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn project_problem(folder: &Path, error: &BusinessError) -> String {
    match error {
        BusinessError::InvalidInput(_) => format!("{} is not a folder", folder.display()),
        _ => format!("{} does not exist", folder.display()),
    }
}
