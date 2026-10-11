use std::path::PathBuf;

use clap::Parser;

pub const DEFAULT_SOURCE_DIR: &str = "images/source";

/// Recreate a target image as a photo mosaic built from a folder of source media.
#[derive(Parser, Debug)]
#[command(name = "mosaicer", version, about)]
pub struct Args {
    /// Image to recreate (webp, png or jpg).
    #[arg(short, long, value_name = "FILE")]
    pub target_image: PathBuf,

    /// Folder with at least 10 source files (jpg, png, webp or mp4).
    /// Defaults to `images/source` relative to the current directory.
    #[arg(short, long, value_name = "DIR", default_value = DEFAULT_SOURCE_DIR)]
    pub source_images: PathBuf,
}
