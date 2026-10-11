use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Debug, Clone)]
pub struct MediaFile {
    pub path: PathBuf,
    pub kind: MediaKind,
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "heic", "heif"];
const HEIC_EXTENSIONS: &[&str] = &["heic", "heif"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4"];

fn ext(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

pub fn classify(path: &Path) -> Option<MediaKind> {
    let ext = ext(path)?;
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaKind::Image)
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

pub fn is_heic(path: &Path) -> bool {
    ext(path).is_some_and(|e| HEIC_EXTENSIONS.contains(&e.as_str()))
}

pub struct ScanResult {
    pub supported: Vec<MediaFile>,
    pub skipped: Vec<PathBuf>,
}

impl ScanResult {
    pub fn images(&self) -> impl Iterator<Item = &MediaFile> {
        self.supported.iter().filter(|m| m.kind == MediaKind::Image)
    }
    pub fn videos(&self) -> impl Iterator<Item = &MediaFile> {
        self.supported.iter().filter(|m| m.kind == MediaKind::Video)
    }
}

/// Non-recursive scan; hidden files ignored, unsupported extensions reported as skipped.
pub fn scan_dir(dir: &Path) -> Result<ScanResult> {
    let mut supported = Vec::new();
    let mut skipped = Vec::new();

    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden || !path.is_file() {
            continue;
        }
        match classify(&path) {
            Some(kind) => supported.push(MediaFile { path, kind }),
            None => skipped.push(path),
        }
    }

    supported.sort_by(|a, b| a.path.cmp(&b.path));
    skipped.sort();
    Ok(ScanResult { supported, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_case_insensitive_extension() {
        assert_eq!(classify(Path::new("a.JPG")), Some(MediaKind::Image));
        assert_eq!(classify(Path::new("a.webp")), Some(MediaKind::Image));
        assert_eq!(classify(Path::new("a.HEIC")), Some(MediaKind::Image));
        assert_eq!(classify(Path::new("a.MP4")), Some(MediaKind::Video));
        assert_eq!(classify(Path::new("a.gif")), None);
        assert_eq!(classify(Path::new("noext")), None);
        assert!(is_heic(Path::new("x.heic")));
        assert!(!is_heic(Path::new("x.jpg")));
    }
}
