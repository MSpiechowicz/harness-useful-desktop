//! The answers of `harness-knowledge docs …` for a review, read into the app's types.

use serde::Deserialize;
use serde_json::Value;

use crate::error::{BusinessError, Result};
use crate::model::{
    FileDiff, GitFileStatus, Hunk, KnowledgeComment, KnowledgeCommentChanged, KnowledgeCommentsDeleted,
    KnowledgeCompare, KnowledgeId, KnowledgeOutcome, KnowledgeRefusalCode, KnowledgeSectionDiff, KnowledgeTransitioned,
};

/// `comment` with `lineFrom` and `lineTo` under the names a review sends them with.
pub(super) fn renamed_lines(comment: &mut Value) {
    let Value::Object(fields) = comment else {
        return;
    };

    for (stored, sent) in [("lineFrom", "from"), ("lineTo", "to")] {
        if !fields.contains_key(sent)
            && let Some(line) = fields.remove(stored)
        {
            fields.insert(sent.to_owned(), line);
        }
    }
}

pub(super) fn comment_required<T>() -> KnowledgeOutcome<T> {
    KnowledgeOutcome::Refused {
        code: KnowledgeRefusalCode::CommentRequired,
        message: "say what should change".to_owned(),
    }
}

pub(super) fn read_transitioned(data: &Value) -> Result<KnowledgeTransitioned> {
    serde_json::from_value(data.clone())
        .map_err(|_| BusinessError::Protocol("harness-knowledge answered a state this app cannot read".to_owned()))
}

pub(super) fn read_changed(data: &Value) -> Result<KnowledgeCommentChanged> {
    let comment = data
        .get("comment")
        .filter(|comment| !comment.is_null())
        .map(|comment| serde_json::from_value::<KnowledgeComment>(comment.clone()))
        .transpose()
        .map_err(|_| BusinessError::Protocol("harness-knowledge answered a comment this app cannot read".to_owned()))?;

    Ok(KnowledgeCommentChanged { comment })
}

pub(super) fn read_deleted(data: &Value) -> Result<KnowledgeCommentsDeleted> {
    serde_json::from_value(data.clone()).map_err(|_| {
        BusinessError::Protocol("harness-knowledge answered deleted comments this app cannot read".to_owned())
    })
}

/// One section of a compare as the server prints it: a [`FileDiff`] whose status may be one of the
/// section words (`removed`, `changed`, `same`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SectionDiff {
    path: String,
    /// Which of the sections that share `path` it is (from 1); the first from a server that does not say.
    #[serde(default)]
    occurrence: Option<u32>,
    #[serde(default)]
    old_path: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    hunks: Vec<Hunk>,
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
    #[serde(default)]
    truncated: bool,
}

impl SectionDiff {
    /// The section, `None` for one that did not change.
    fn into_section(self) -> Option<KnowledgeSectionDiff> {
        let status = match self.status.as_str() {
            "same" => return None,
            "added" => GitFileStatus::Added,
            "deleted" | "removed" => GitFileStatus::Deleted,
            _ => GitFileStatus::Modified,
        };

        let diff = FileDiff {
            path: self.path,
            old_path: self.old_path,
            status,
            staged: false,
            is_binary: false,
            hunks: self.hunks,
            truncated: self.truncated,
            additions: self.additions,
            deletions: self.deletions,
        };

        Some(KnowledgeSectionDiff {
            diff,
            occurrence: self.occurrence.filter(|occurrence| *occurrence >= 1).unwrap_or(1),
        })
    }
}

pub(super) fn read_compare(data: &Value) -> Result<KnowledgeCompare> {
    let sections = data.get("sections").or_else(|| data.get("files"));
    let sections: Vec<SectionDiff> = sections
        .and_then(|sections| serde_json::from_value(sections.clone()).ok())
        .ok_or_else(|| {
            BusinessError::Protocol("harness-knowledge docs compare answered sections this app cannot read".to_owned())
        })?;

    let id = |name: &str| {
        data.get(name)
            .and_then(|id| serde_json::from_value::<KnowledgeId>(id.clone()).ok())
            .filter(KnowledgeId::is_document)
    };

    Ok(KnowledgeCompare {
        live: id("live"),
        pending: id("pending"),
        revision: data
            .get("revision")
            .and_then(Value::as_i64)
            .filter(|revision| *revision >= 0),
        sections: sections.into_iter().filter_map(SectionDiff::into_section).collect(),
        truncated: data.get("truncated").and_then(Value::as_bool).unwrap_or(false),
    })
}
