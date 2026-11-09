//! What the page reports about the chats ([`bounded`]) and the chat title as a notification body
//! ([`notification_body`]). A chat's title is text the model or the user wrote, so everything here
//! treats it as untrusted and as possibly secret.

use std::collections::HashMap;

use harness_business::model::{ChatAlertsView, display_text};
use harness_business::security::redact_text;

/// The most chats whose titles are kept.
pub const MAX_TITLES: usize = 64;

/// The longest chat key kept, in bytes. Chat keys are UUIDs, 36 bytes.
pub const MAX_KEY_BYTES: usize = 128;

/// The most characters of a chat title that are kept.
pub const MAX_TITLE_CHARS: usize = 200;

/// The most characters of a notification body.
pub const MAX_BODY_CHARS: usize = 80;

/// `view` within bounds. What is over a limit is dropped: a key that is too long, a title with
/// nothing in it, and every title past the [`MAX_TITLES`] first keys in order. Titles are trimmed
/// and cut at [`MAX_TITLE_CHARS`].
pub fn bounded(view: ChatAlertsView) -> ChatAlertsView {
    let shown = view.shown.filter(|key| fits_key(key));

    let mut titles: Vec<(String, String)> = view
        .titles
        .into_iter()
        .filter(|(key, _)| fits_key(key))
        .map(|(key, title)| (key, title.trim().chars().take(MAX_TITLE_CHARS).collect::<String>()))
        .filter(|(_, title)| !title.trim().is_empty())
        .collect();

    titles.sort();
    titles.truncate(MAX_TITLES);

    ChatAlertsView {
        shown,
        titles: titles.into_iter().collect::<HashMap<_, _>>(),
    }
}

fn fits_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_KEY_BYTES
}

/// Whether `key` has the form of a chat's key: not empty, at most [`MAX_KEY_BYTES`] bytes, ASCII
/// letters, digits and `-` only. A notification's data comes back with a click, so it is checked.
pub fn is_chat_key(key: &str) -> bool {
    fits_key(key) && key.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// `title` as the body of an OS notification, whose server may read markup (some Linux ones do):
/// control and invisible characters are dropped, then secrets are masked ([`redact_text`], a name
/// can hold anything its author typed), `<`, `>` and `&` are dropped and the masking runs once
/// more (a dropped character can rejoin a split secret), whitespace becomes single spaces, and a title over [`MAX_BODY_CHARS`] characters ends in an ellipsis. `None` when nothing
/// usable is left.
pub fn notification_body(title: &str) -> Option<String> {
    // Line breaks and tabs separate words, so they become spaces before the rest is dropped.
    let spaced: String = title
        .chars()
        .take(MAX_TITLE_CHARS)
        .map(|character| if character.is_whitespace() { ' ' } else { character })
        .collect();

    // Invisible characters go first, so none can split a secret and hide it from the masking.
    let visible = display_text(&spaced, MAX_TITLE_CHARS);

    // Dropping markup characters can join the two halves of a secret that one of them split, so
    // the masking runs again on what is left, before anything is cut.
    let dropped: String = redact_text(&visible)
        .chars()
        .filter(|character| !matches!(character, '<' | '>' | '&'))
        .collect();
    let plain = redact_text(&dropped);

    let words = plain.split_whitespace().collect::<Vec<_>>().join(" ");

    if words.is_empty() {
        return None;
    }

    if words.chars().count() <= MAX_BODY_CHARS {
        return Some(words);
    }

    let kept: String = words.chars().take(MAX_BODY_CHARS - 1).collect();

    Some(format!("{}…", kept.trim_end()))
}
