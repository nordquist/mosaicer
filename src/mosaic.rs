use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{ensure, Context, Result};

use crate::cli::Args;
use crate::config::{
    BLEND_ALPHA, EXCLUSION_RADIUS, GRID_COLS, MAX_OUTPUT_PX, PENALTY_FACTOR, SEED, TILE_PX,
};
use crate::matcher::MatchParams;
use crate::{compose, decode, grid, index, matcher, report, validate};

pub fn run(args: &Args) -> Result<PathBuf> {
    validate::target_image(&args.target_image)?;
    let scan = validate::source_dir(&args.source_images)?;
    println!("target:  {}", args.target_image.display());
    println!(
        "sources: {} ({} images, {} videos, {} unsupported skipped)",
        args.source_images.display(),
        scan.images().count(),
        scan.videos().count(),
        scan.skipped.len()
    );

    let t = Instant::now();
    let (tiles, stats) = index::build(&scan, &args.source_images)?;
    report::print_index(&stats, &tiles);
    ensure!(!tiles.is_empty(), "no source image could be decoded");
    println!("         indexed in {:.1}s", t.elapsed().as_secs_f32());

    let target = decode::decode(&args.target_image).context("decode target")?;
    let grid = grid::build(&target, GRID_COLS)?;
    ensure!(
        grid.cols * TILE_PX <= MAX_OUTPUT_PX && grid.rows * TILE_PX <= MAX_OUTPUT_PX,
        "output would exceed {MAX_OUTPUT_PX}px ({}x{} tiles of {TILE_PX}px)",
        grid.cols,
        grid.rows
    );

    let t = Instant::now();
    let params = MatchParams {
        exclusion_radius: EXCLUSION_RADIUS,
        penalty_factor: PENALTY_FACTOR,
        seed: SEED,
    };
    let assignment = matcher::assign(&grid, &tiles, &params);
    report::print_quality(&grid, &tiles, &assignment);
    println!("         matched in {:.2}s", t.elapsed().as_secs_f32());

    let out = compose::render(&grid, &tiles, &assignment, TILE_PX, BLEND_ALPHA);
    let out_path = output_path(&args.target_image);
    out.save(&out_path)
        .with_context(|| format!("write {}", out_path.display()))?;
    println!(
        "output:  {} ({}x{}, blend {BLEND_ALPHA})",
        out_path.display(),
        out.width(),
        out.height()
    );
    Ok(out_path)
}

fn output_path(target: &Path) -> PathBuf {
    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("target");
    target.with_file_name(format!("{stem}-mosaic.png"))
}
