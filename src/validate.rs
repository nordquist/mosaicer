use std::path::Path;

use anyhow::{bail, ensure, Result};

use crate::media::{self, MediaKind, ScanResult};

pub const MIN_SOURCE_FILES: usize = 10;

pub fn target_image(path: &Path) -> Result<()> {
    ensure!(path.is_file(), "target image not found: {}", path.display());
    match media::classify(path) {
        Some(MediaKind::Image) if !media::is_heic(path) => Ok(()),
        _ => bail!("target image must be webp, png or jpg: {}", path.display()),
    }
}

pub fn source_dir(dir: &Path) -> Result<ScanResult> {
    ensure!(dir.is_dir(), "source folder not found: {}", dir.display());
    let scan = media::scan_dir(dir)?;
    ensure!(
        scan.supported.len() >= MIN_SOURCE_FILES,
        "source folder needs at least {MIN_SOURCE_FILES} jpg/png/webp/heic/mp4 files, found {} in {}",
        scan.supported.len(),
        dir.display()
    );
    Ok(scan)
}
