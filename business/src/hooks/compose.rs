//! The text hooks add to prompts (see "Prompt text" in the format doc).
//!
//! Context goes in front of the user's text:
//!
//! ```text
//! <harness-hook-context>
//! <hook name="NAME">TEXT</hook>
//! </harness-hook-context>
//!
//! USER TEXT
//! ```
//!
//! A follow-up prompt is `<harness-hook-followup name="NAME">TEXT</harness-hook-followup>`
//! elements joined by a blank line. In `NAME` `& < > "` are escaped; in `TEXT` a `<` that begins
//! `<hook`, `</hook`, `<harness-hook` or `</harness-hook` (ASCII case-insensitive) is written as
//! `&lt;`, so a hook's text can never close or forge an element. Each block holds at most
//! [`MAX_COMPOSED_BYTES`] of elements: an element that does not fit is dropped (and logged) and
//! later smaller ones may still fit. When the message with its context would exceed
//! [`MAX_ACTION_TEXT_BYTES`], the last elements (lowest in run order) are dropped until it fits.

use crate::model::MAX_ACTION_TEXT_BYTES;

/// First line of a context block.
pub const CONTEXT_OPEN: &str = "<harness-hook-context>";

/// Last line of a context block.
pub const CONTEXT_CLOSE: &str = "</harness-hook-context>";

/// Largest composed context block or follow-up prompt, counting its elements and the line breaks
/// between them (the wrapper lines of a context block are not counted).
pub const MAX_COMPOSED_BYTES: usize = 8 * 1024;

const ELEMENT_OPEN: &str = "<hook name=\"";
const ELEMENT_CLOSE: &str = "</hook>";
const FOLLOW_UP_OPEN: &str = "<harness-hook-followup name=\"";
const FOLLOW_UP_CLOSE: &str = "</harness-hook-followup>";

/// What one hook contributes to a prompt: an `addContext` or `followUp` text, or a command's
/// output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPart {
    /// The hook's id.
    pub hook_id: String,
    /// The hook's name (shown in the element).
    pub hook_name: String,
    /// The text.
    pub text: String,
    /// Run order of the hook.
    pub(crate) order: (u8, [u32; 7], String),
}

impl ResolvedPart {
    /// A part of the hook `hook_id` named `hook_name`, ordered by id alone.
    pub fn new(hook_id: impl Into<String>, hook_name: impl Into<String>, text: impl Into<String>) -> Self {
        let hook_id = hook_id.into();

        Self {
            order: (0, [0; 7], hook_id.clone()),
            hook_id,
            hook_name: hook_name.into(),
            text: text.into(),
        }
    }
}

/// One `<hook>` element of a context block, unescaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookContextItem {
    /// The hook's name.
    pub name: String,
    /// What it added.
    pub text: String,
}

/// The context block for `parts` (in the order given), or `None` when no element fits.
pub fn compose_context(parts: &[ResolvedPart]) -> Option<String> {
    context_block(&context_elements(parts))
}

/// The `<hook>` elements of `parts` that fit in [`MAX_COMPOSED_BYTES`], with their hook ids.
fn context_elements(parts: &[ResolvedPart]) -> Vec<(&str, String)> {
    let elements = parts
        .iter()
        .map(|part| {
            let element = format!(
                "{ELEMENT_OPEN}{}\">{}{ELEMENT_CLOSE}",
                escape_name(&part.hook_name),
                escape_text(&part.text)
            );
            (part.hook_id.as_str(), element)
        })
        .collect();

    within_cap(elements, 1)
}

fn context_block(elements: &[(&str, String)]) -> Option<String> {
    if elements.is_empty() {
        return None;
    }

    let lines: Vec<&str> = elements.iter().map(|(_, element)| element.as_str()).collect();

    Some(format!("{CONTEXT_OPEN}\n{}\n{CONTEXT_CLOSE}", lines.join("\n")))
}

/// `message` with the context block of `parts` in front of it (see [`compose_prompt`]).
pub fn with_hook_context(message: &str, parts: &[ResolvedPart]) -> String {
    compose_prompt(message, parts).0
}

/// `message` with the context block of `parts` in front of it, and the ids of the hooks whose
/// context it carries. When the result would exceed [`MAX_ACTION_TEXT_BYTES`], the last elements
/// are dropped (and logged) until it fits; the message is unchanged when none fits.
pub fn compose_prompt(message: &str, parts: &[ResolvedPart]) -> (String, Vec<String>) {
    let mut elements = context_elements(parts);

    while let Some(block) = context_block(&elements) {
        let composed = format!("{block}\n\n{message}");

        if composed.len() <= MAX_ACTION_TEXT_BYTES {
            let kept = elements.iter().map(|(hook_id, _)| (*hook_id).to_owned()).collect();
            return (composed, kept);
        }

        if let Some((hook_id, _)) = elements.pop() {
            tracing::warn!(hook = hook_id, "hook context left out: the message would be too long");
        }
    }

    (message.to_owned(), Vec::new())
}

/// The follow-up prompt for `parts` (in the order given), or `None` when no element fits.
pub fn compose_follow_up(parts: &[ResolvedPart]) -> Option<String> {
    let elements = parts
        .iter()
        .map(|part| {
            let element = format!(
                "{FOLLOW_UP_OPEN}{}\">{}{FOLLOW_UP_CLOSE}",
                escape_name(&part.hook_name),
                escape_text(&part.text)
            );
            (part.hook_id.as_str(), element)
        })
        .collect();

    let kept: Vec<String> = within_cap(elements, 2)
        .into_iter()
        .map(|(_, element)| element)
        .collect();

    (!kept.is_empty()).then(|| kept.join("\n\n"))
}

/// The elements that fit in [`MAX_COMPOSED_BYTES`] with `separator` bytes between them, in order;
/// each one that does not fit is logged by hook id.
fn within_cap(elements: Vec<(&str, String)>, separator: usize) -> Vec<(&str, String)> {
    let mut kept: Vec<(&str, String)> = Vec::new();
    let mut used = 0;

    for (hook_id, element) in elements {
        let cost = element.len() + if kept.is_empty() { 0 } else { separator };

        if used + cost > MAX_COMPOSED_BYTES {
            tracing::warn!(hook = hook_id, "hook text left out: over the 8 KiB limit");
            continue;
        }

        used += cost;
        kept.push((hook_id, element));
    }

    kept
}

/// `name` with `& < > "` escaped.
pub fn escape_name(name: &str) -> String {
    name.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The markers a `<` in a text must not begin (lowercase, without the `<`).
const MARKERS: [&str; 4] = ["hook", "/hook", "harness-hook", "/harness-hook"];

/// True when `rest` (what follows a `<`) begins a marker, ASCII case-insensitively.
fn begins_marker(rest: &str) -> bool {
    MARKERS.iter().any(|marker| {
        rest.as_bytes()
            .get(..marker.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(marker.as_bytes()))
    })
}

/// `text` with every `<` that begins a hook marker written as `&lt;`.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for (index, character) in text.char_indices() {
        if character == '<' && begins_marker(&text[index + 1..]) {
            out.push_str("&lt;");
        } else {
            out.push(character);
        }
    }

    out
}

fn unescape_name(name: &str) -> String {
    name.replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn unescape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("&lt;") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];

        out.push_str(if begins_marker(after) { "<" } else { "&lt;" });
        rest = after;
    }

    out.push_str(rest);
    out
}

/// The elements of a leading context block and the user's text after it; `None` when `text` does
/// not start with a well-formed block (the wrapper, one or more `<hook>` elements each on its own
/// line, then a blank line and the message, or nothing).
pub fn parse_hook_context(text: &str) -> Option<(Vec<HookContextItem>, &str)> {
    let mut rest = text.strip_prefix(CONTEXT_OPEN)?.strip_prefix('\n')?;
    let mut items = Vec::new();

    loop {
        rest = rest.strip_prefix(ELEMENT_OPEN)?;

        let name_end = rest.find('"')?;
        let name = &rest[..name_end];
        rest = rest[name_end..].strip_prefix("\">")?;

        let text_end = rest.find(ELEMENT_CLOSE)?;
        items.push(HookContextItem {
            name: unescape_name(name),
            text: unescape_text(&rest[..text_end]),
        });
        rest = rest[text_end + ELEMENT_CLOSE.len()..].strip_prefix('\n')?;

        if let Some(after) = rest.strip_prefix(CONTEXT_CLOSE) {
            rest = after;
            break;
        }
    }

    match rest.strip_prefix("\n\n") {
        Some(message) => Some((items, message)),
        None if rest.is_empty() => Some((items, rest)),
        None => None,
    }
}

/// `text` without a leading hook context block: what the user wrote. Text that does not start
/// with a well-formed block is returned unchanged.
pub fn strip_hook_context(text: &str) -> &str {
    parse_hook_context(text).map_or(text, |(_, message)| message)
}
