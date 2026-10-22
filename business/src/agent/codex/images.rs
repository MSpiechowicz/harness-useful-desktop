//! Images of a message as files Codex reads (`localImage` input): saved below
//! [`SessionOptions::attachments_dir`](crate::agent::SessionOptions::attachments_dir) (`0700`),
//! by default `attachments` in the app's data folder, each in a folder of its own and pruned after
//! [`KEEP`](crate::attachments::KEEP) like every saved attachment.

use std::path::{Path, PathBuf};

use super::Chat;
use crate::app_dirs::local_data_dir;
use crate::attachments::save_attachment;
use crate::codex_rpc::wire::items::UserInput;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::ensure_private_dir;
use crate::model::ImageAttachment;

impl Chat {
    /// `text` and `images` as Codex input: a skill the text calls (`skills.rs`), the images saved
    /// as files Codex reads.
    pub(super) async fn input(&self, text: &str, images: &[ImageAttachment]) -> Result<Vec<UserInput>> {
        let mut input = Vec::new();

        if !text.is_empty() {
            input.push(UserInput::text(text));
        }

        input.extend(self.skill_input(text));

        if images.is_empty() {
            return Ok(input);
        }

        let root = attachments_root(self.attachments_dir.as_ref())?;
        let images = images.to_vec();

        let paths = tokio::task::spawn_blocking(move || save_images(&root, &images))
            .await
            .map_err(|error| BusinessError::Internal(format!("saving the images failed: {error}")))??;

        input.extend(paths.into_iter().map(|path| UserInput::LocalImage { path }));

        Ok(input)
    }
}

/// `configured`, or `attachments` in the app's data folder.
fn attachments_root(configured: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = configured {
        return Ok(dir.clone());
    }

    let env = Env::from_process()?;
    let data = local_data_dir(&env).ok_or_else(|| BusinessError::NotFound("the app's data folder".to_owned()))?;

    Ok(data.join("attachments"))
}

/// Saves `images` below `root` (private to the user); their paths.
fn save_images(root: &Path, images: &[ImageAttachment]) -> Result<Vec<String>> {
    ensure_private_dir(root, "create the attachment folder")?;

    images
        .iter()
        .map(|image| {
            let name = format!("image.{}", extension(&image.mime_type));
            save_attachment(root, &name, &image.data).map(|saved| saved.path)
        })
        .collect()
}

/// The file extension of an image type.
fn extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}
