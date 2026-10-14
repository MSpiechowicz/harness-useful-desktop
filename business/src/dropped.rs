//! Reads what a user dropped onto the window: images are read, every other file is named by path.
//!
//! The paths come from the operating system's drag and drop, never from the webview. Only files
//! whose content is a PNG, JPEG, GIF or WebP are read into the webview; any other file stays on
//! disk and the agent opens it by path.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::model::{AttachedFile, DroppedImage, MAX_ACTION_IMAGES};

/// Largest dropped file that is read (20 MiB); the webview scales it down before sending.
pub const MAX_DROPPED_BYTES: u64 = 20 * 1024 * 1024;

/// What a drop held: the images (at most [`MAX_ACTION_IMAGES`]), the other files, and the names
/// of items that are neither (folders, unreadable files).
pub fn read_dropped<P: AsRef<Path>>(paths: &[P]) -> (Vec<DroppedImage>, Vec<AttachedFile>, Vec<String>) {
    let mut images = Vec::new();
    let mut files = Vec::new();
    let mut skipped = Vec::new();

    for path in paths {
        let path = path.as_ref();
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());

        let image = (images.len() < MAX_ACTION_IMAGES).then(|| read_image(path)).flatten();
        if let Some((mime_type, bytes)) = image {
            images.push(DroppedImage {
                name,
                mime_type: mime_type.to_owned(),
                data: STANDARD.encode(bytes),
            });
        } else if let Some(bytes) = file_size(path) {
            files.push(AttachedFile {
                name,
                path: path.to_string_lossy().into_owned(),
                bytes,
            });
        } else {
            skipped.push(name);
        }
    }

    (images, files, skipped)
}

/// The size of a regular, readable file.
fn file_size(path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;

    metadata.is_file().then_some(metadata.len())
}

fn read_image(path: &Path) -> Option<(&'static str, Vec<u8>)> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_DROPPED_BYTES {
        return None;
    }

    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).ok()?);
    file.take(MAX_DROPPED_BYTES).read_to_end(&mut bytes).ok()?;

    Some((sniff(&bytes)?, bytes))
}

/// The image type a file's first bytes announce.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("image/webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_four_image_types_by_content() {
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            Some("image/png")
        );
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a.."), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert_eq!(sniff(b"# not an image"), None);
        assert_eq!(sniff(&[]), None);
    }

    #[test]
    fn returns_images_reads_nothing_else_and_names_what_it_cannot_use() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        let log = dir.path().join("build.log");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2]).unwrap();
        std::fs::write(&log, "error: boom").unwrap();

        let (images, files, skipped) =
            read_dropped(&[png, log.clone(), dir.path().to_path_buf(), dir.path().join("gone.jpg")]);

        assert_eq!(images.len(), 1);
        assert_eq!(
            (images[0].name.as_str(), images[0].mime_type.as_str()),
            ("shot.png", "image/png")
        );
        assert_eq!(STANDARD.decode(&images[0].data).unwrap().len(), 10);
        assert_eq!(
            files,
            vec![AttachedFile {
                name: "build.log".to_owned(),
                path: log.to_string_lossy().into_owned(),
                bytes: 11
            }]
        );
        assert_eq!(skipped.len(), 2);
    }

    #[test]
    fn images_beyond_the_limit_become_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = (0..=MAX_ACTION_IMAGES)
            .map(|index| {
                let path = dir.path().join(format!("{index}.jpg"));
                std::fs::write(&path, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
                path
            })
            .collect();

        let (images, files, skipped) = read_dropped(&paths);

        assert_eq!(images.len(), MAX_ACTION_IMAGES);
        assert_eq!(files.len(), 1);
        assert!(skipped.is_empty());
    }
}
