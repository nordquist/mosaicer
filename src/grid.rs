use anyhow::{ensure, Result};
use image::RgbImage;

use crate::color::Oklab;
use crate::descriptor::{self, Descriptor, Rect};

pub struct Cell {
    pub col: u32,
    pub row: u32,
    pub descriptor: Descriptor,
    pub mean: Oklab,
    pub saliency: f32,
}

pub struct Grid {
    pub cols: u32,
    pub rows: u32,
    /// Row-major.
    pub cells: Vec<Cell>,
}

impl Grid {
    pub fn index(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }
}

/// Square-ish cells: `cols` across, rows chosen to preserve the target aspect ratio.
pub fn build(target: &RgbImage, cols: u32) -> Result<Grid> {
    ensure!(cols >= 1, "grid needs at least one column");
    let cols = cols.min(target.width());
    let cell_w = target.width() as f32 / cols as f32;
    let rows = ((target.height() as f32 / cell_w).round() as u32).clamp(1, target.height());
    let cell_h = target.height() as f32 / rows as f32;

    let mut cells = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let x0 = (col as f32 * cell_w).round() as u32;
            let x1 = ((col + 1) as f32 * cell_w).round() as u32;
            let y0 = (row as f32 * cell_h).round() as u32;
            let y1 = ((row + 1) as f32 * cell_h).round() as u32;
            let rect = Rect {
                x: x0,
                y: y0,
                w: (x1 - x0).max(1),
                h: (y1 - y0).max(1),
            };
            let d = descriptor::describe(target, rect);
            cells.push(Cell {
                col,
                row,
                mean: descriptor::mean_oklab(&d),
                saliency: descriptor::saliency(&d),
                descriptor: d,
            });
        }
    }
    Ok(Grid { cols, rows, cells })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn rows_follow_aspect_ratio() {
        let img = RgbImage::from_pixel(448, 684, Rgb([0, 0, 0]));
        let g = build(&img, 48).unwrap();
        assert_eq!((g.cols, g.rows), (48, 73));
        assert_eq!(g.cells.len(), 48 * 73);
        assert_eq!(g.index(0, 1), 48);
    }

    #[test]
    fn cols_clamped_to_width() {
        let img = RgbImage::from_pixel(5, 5, Rgb([0, 0, 0]));
        let g = build(&img, 48).unwrap();
        assert_eq!((g.cols, g.rows), (5, 5));
    }
}
