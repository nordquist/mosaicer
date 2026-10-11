use image::RgbImage;

use crate::color::oklab_to_srgb;
use crate::grid::Grid;
use crate::index::Tile;
use crate::matcher::Assignment;

/// Paste each cell's tile, optionally blended toward the cell's mean colour.
pub fn render(
    grid: &Grid,
    tiles: &[Tile],
    assignment: &Assignment,
    tile_px: u32,
    alpha: f32,
) -> RgbImage {
    let mut out = RgbImage::new(grid.cols * tile_px, grid.rows * tile_px);
    let alpha = alpha.clamp(0.0, 1.0);
    let tile_px = tile_px as usize;
    let out_w = out.width() as usize;

    for (i, cell) in grid.cells.iter().enumerate() {
        let thumb = &tiles[assignment.tile_of_cell[i]].thumb;
        let tint = oklab_to_srgb(cell.mean);
        let ox = cell.col as usize * tile_px;
        let oy = cell.row as usize * tile_px;
        let src = thumb.as_raw();
        let dst = out.as_mut();
        for y in 0..tile_px {
            let s = &src[y * tile_px * 3..(y + 1) * tile_px * 3];
            let d = &mut dst[((oy + y) * out_w + ox) * 3..((oy + y) * out_w + ox + tile_px) * 3];
            for (dp, sp) in d.chunks_exact_mut(3).zip(s.chunks_exact(3)) {
                for k in 0..3 {
                    dp[k] = ((1.0 - alpha) * sp[k] as f32 + alpha * tint[k] as f32).round() as u8;
                }
            }
        }
    }
    out
}
