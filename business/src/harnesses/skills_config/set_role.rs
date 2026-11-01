//! Changing the model of one Harness Skills role in `harness-useful-skills/models.yml` of Claude
//! Code, OpenCode (user and project scope) and Codex.
//!
//! Harness Skills owns the file and adds the rows of new roles itself, so only an existing row
//! changes, and only its value: the indent, the quotes, a trailing comment, the line ending and
//! every other line stay as they are. The file is never created. The result must have the same
//! rows as before, except the changed value, otherwise the edit is refused.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::models_path;
use super::selector::validate;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harnesses::claude::config_dir as claude_dir;
use crate::harnesses::codex::codex_home;
use crate::harnesses::opencode::{PROJECT_DIR_NAME, config_dir as opencode_dir};
use crate::line_edit::{Lines, content, edit_text_file, is_skipped};
use crate::model::{ConfigLayer, HarnessId};

/// Harness Skills refuses a models file larger than this.
const MAX_BYTES: u64 = 16 * 1024;

const HEADER: &str = "modelRoles:";

/// A comment that ends a line: whitespace, then `#`.
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+#.*$").expect("static regex is valid"));

/// `  role: "selector"` without its comment, like Harness Skills' own parser reads a row.
static ROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s+([a-z]+):\s*(["']?)([^"'\s#][^"'\s]*)(["']?)$"#).expect("static regex is valid")
});

static ROLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]+$").expect("static regex is valid"));

/// Sets `role` to `selector` in the models file of `harness` in `scope` (`project` is the
/// authorized project for the project scope). `invalidInput` for a role name or a selector that
/// Harness Skills would not accept, `notFound` for a file or a role row that is not there,
/// `forbidden` for a project file that resolves outside the project, `conflict` when the file
/// changed meanwhile. See [`set_skills_role_with`].
pub fn set_skills_role(
    env: &Env,
    harness: HarnessId,
    scope: ConfigLayer,
    project: Option<&Path>,
    role: &str,
    selector: &str,
) -> Result<()> {
    set_skills_role_with(env, harness, scope, project, role, selector, || {})
}

/// [`set_skills_role`], running `before_write` after the edit is planned and before the file is
/// replaced, where a change made meanwhile by another program is caught and refused.
pub fn set_skills_role_with(
    env: &Env,
    harness: HarnessId,
    scope: ConfigLayer,
    project: Option<&Path>,
    role: &str,
    selector: &str,
    before_write: impl FnOnce(),
) -> Result<()> {
    if !ROLE.is_match(role) {
        return Err(BusinessError::InvalidInput(format!("{role} is not a role name")));
    }

    validate(harness, selector)?;

    let (path, within) = models_file(env, harness, scope, project)?;

    edit_text_file(
        &path,
        "the Harness Skills model roles",
        MAX_BYTES,
        within,
        |text| set_value(text, role, selector),
        before_write,
    )
}

/// The models file of `harness` in `scope`, and for a project file the project it must stay in.
fn models_file<'a>(
    env: &Env,
    harness: HarnessId,
    scope: ConfigLayer,
    project: Option<&'a Path>,
) -> Result<(PathBuf, Option<&'a Path>)> {
    let scope_dir = match (harness, scope) {
        (HarnessId::Claude, ConfigLayer::User) => claude_dir(env),
        (HarnessId::Codex, ConfigLayer::User) => codex_home(env),
        (HarnessId::OpenCode, ConfigLayer::User) => opencode_dir(env),
        (HarnessId::OpenCode, ConfigLayer::Project) => {
            let project = project
                .ok_or_else(|| BusinessError::InvalidInput("the project scope needs an open project".to_owned()))?;

            return Ok((models_path(&project.join(PROJECT_DIR_NAME)), Some(project)));
        }
        _ => {
            return Err(BusinessError::Unsupported(
                "this harness has no Harness Skills model roles in that scope".to_owned(),
            ));
        }
    };

    Ok((models_path(&scope_dir), None))
}

/// One `role: value` row of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    line: usize,
    role: String,
    value: String,
}

/// `text` with the value of the `role` row replaced by `selector`.
fn set_value(text: &str, role: &str, selector: &str) -> Result<String> {
    let mut lines = Lines::new(text);
    let old = rows(&lines)?;

    let found: Vec<&Row> = old.iter().filter(|row| row.role == role).collect();

    let [row] = found.as_slice() else {
        return Err(match found.len() {
            0 => BusinessError::NotFound(format!("the {role} row of the model roles")),
            _ => unsupported(),
        });
    };

    let current = content(&lines.lines[row.line]).to_owned();
    let (head, tail) = split_value(&current).ok_or_else(unsupported)?;

    lines.set_content(row.line, &format!("{head}{selector}{tail}"));

    // What the line search did not see shows up here: only that value may differ.
    let expected: Vec<Row> = old
        .iter()
        .map(|row| {
            if row.role == role {
                Row {
                    value: selector.to_owned(),
                    ..row.clone()
                }
            } else {
                row.clone()
            }
        })
        .collect();

    if rows(&lines)? != expected {
        return Err(unsupported());
    }

    Ok(lines.text())
}

/// The line before its value and after it (closing quote, spaces and comment).
fn split_value(line: &str) -> Option<(&str, &str)> {
    let stripped = strip_comment(line);
    let value = ROW.captures(stripped)?.get(3)?;

    Some((&line[..value.start()], &line[value.end()..]))
}

/// The line without a trailing comment and trailing spaces.
fn strip_comment(line: &str) -> &str {
    let cut = COMMENT.find(line).map_or(line, |comment| &line[..comment.start()]);

    cut.trim_end()
}

/// The rows below `modelRoles:` in the strict shape Harness Skills reads: comments and blank
/// lines, the header and indented `role: selector` rows, nothing else.
fn rows(lines: &Lines) -> Result<Vec<Row>> {
    let mut found = Vec::new();
    let mut in_roles = false;

    for (index, line) in lines.lines.iter().enumerate() {
        let text = strip_comment(content(line));

        if is_skipped(text) {
            continue;
        }

        if text == HEADER {
            in_roles = true;

            continue;
        }

        let Some(captures) = ROW
            .captures(text)
            .filter(|captures| in_roles && captures[2] == captures[4])
        else {
            return Err(unsupported());
        };

        found.push(Row {
            line: index,
            role: captures[1].to_owned(),
            value: captures[3].to_owned(),
        });
    }

    Ok(found)
}

fn unsupported() -> BusinessError {
    BusinessError::Unsupported("the model roles file is laid out in a way that cannot be edited safely".to_owned())
}
