use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};

use crate::media;

/// Decode any supported still image to RGB8 with EXIF orientation applied.
pub fn decode(path: &Path) -> Result<RgbImage> {
    if media::is_heic(path) {
        let tmp = heic_to_jpeg(path)?;
        let result = decode_std(&tmp.0);
        return result;
    }
    decode_std(path)
}

fn decode_std(path: &Path) -> Result<RgbImage> {
    let mut decoder = ImageReader::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .with_guessed_format()?
        .into_decoder()
        .with_context(|| format!("unsupported format {}", path.display()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)
        .with_context(|| format!("decode {}", path.display()))?;
    img.apply_orientation(orientation);
    Ok(img.into_rgb8())
}

/// Temp file removed on drop.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn heic_to_jpeg(path: &Path) -> Result<TempFile> {
    let dir = std::env::temp_dir().join(format!("mosaicer-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("heic");
    let out = dir.join(format!("{name}.jpg"));
    let status = Command::new("sips")
        .args(["-s", "format", "jpeg", "-s", "formatOptions", "95"])
        .arg(path)
        .arg("--out")
        .arg(&out)
        .output()
        .context("run sips (macOS HEIC converter)")?;
    if !status.status.success() {
        bail!(
            "sips failed on {}: {}",
            path.display(),
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    Ok(TempFile(out))
}

#[cfg(not(target_os = "macos"))]
fn heic_to_jpeg(path: &Path) -> Result<TempFile> {
    let _ = Command::new("true");
    bail!(
        "HEIC decoding is only supported on macOS (via sips): {}",
        path.display()
    )
}
