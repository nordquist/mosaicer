use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

use crate::descriptor::distance_sq;
use crate::grid::Grid;
use crate::index::Tile;

pub struct MatchParams {
    pub exclusion_radius: i32,
    pub penalty_factor: f32,
    pub seed: u64,
}

pub struct Assignment {
    /// Tile index per cell (row-major, same order as `grid.cells`).
    pub tile_of_cell: Vec<usize>,
    /// Unpenalised nearest-neighbour distance per cell (sqrt of weighted mean squared Oklab diff).
    pub raw_nn_dist: Vec<f32>,
    pub uses: Vec<u32>,
    pub lambda: f32,
}

/// Greedy, saliency-first assignment with a linear reuse penalty and a local no-repeat radius.
pub fn assign(grid: &Grid, tiles: &[Tile], params: &MatchParams) -> Assignment {
    let n_cells = grid.cells.len();
    let n_tiles = tiles.len();
    assert!(n_tiles > 0, "no tiles to assign");

    let dist: Vec<f32> = grid
        .cells
        .par_iter()
        .flat_map_iter(|cell| {
            tiles
                .iter()
                .map(move |t| distance_sq(&cell.descriptor, &t.descriptor))
        })
        .collect();
    let row = |c: usize| &dist[c * n_tiles..(c + 1) * n_tiles];

    let mut nn_sq: Vec<f32> = (0..n_cells)
        .map(|c| row(c).iter().cloned().fold(f32::INFINITY, f32::min))
        .collect();
    let raw_nn_dist: Vec<f32> = nn_sq.iter().map(|d| d.sqrt()).collect();
    nn_sq.sort_by(|a, b| a.total_cmp(b));
    let lambda = params.penalty_factor * nn_sq[n_cells / 2];

    let mut order: Vec<usize> = (0..n_cells).collect();
    order.shuffle(&mut ChaCha8Rng::seed_from_u64(params.seed));
    order.sort_by(|&a, &b| grid.cells[b].saliency.total_cmp(&grid.cells[a].saliency));

    let mut placed: Vec<Option<usize>> = vec![None; n_cells];
    let mut uses = vec![0u32; n_tiles];
    let mut forbidden: Vec<usize> = Vec::new();

    for &c in &order {
        let cell = &grid.cells[c];
        forbidden.clear();
        let r = params.exclusion_radius;
        for dy in -r..=r {
            for dx in -r..=r {
                let (nc, nr) = (cell.col as i32 + dx, cell.row as i32 + dy);
                if nc < 0 || nr < 0 || nc >= grid.cols as i32 || nr >= grid.rows as i32 {
                    continue;
                }
                if let Some(t) = placed[grid.index(nc as u32, nr as u32)] {
                    forbidden.push(t);
                }
            }
        }

        let costs = row(c);
        let pick = |respect_forbidden: bool| {
            let mut best = None;
            let mut best_cost = f32::INFINITY;
            for (t, &d) in costs.iter().enumerate() {
                if respect_forbidden && forbidden.contains(&t) {
                    continue;
                }
                let cost = d + lambda * uses[t] as f32;
                if cost < best_cost {
                    best_cost = cost;
                    best = Some(t);
                }
            }
            best
        };
        let t = pick(true)
            .or_else(|| pick(false))
            .expect("at least one tile");
        placed[c] = Some(t);
        uses[t] += 1;
    }

    Assignment {
        tile_of_cell: placed
            .into_iter()
            .map(|p| p.expect("every cell placed"))
            .collect(),
        raw_nn_dist,
        uses,
        lambda,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid;
    use crate::index::make_tile;
    use image::{Rgb, RgbImage};

    fn solid(name: &str, rgb: [u8; 3]) -> Tile {
        make_tile(name.into(), &RgbImage::from_pixel(8, 8, Rgb(rgb)))
    }

    #[test]
    fn flat_target_does_not_repeat_within_radius_when_enough_tiles() {
        let target = RgbImage::from_pixel(40, 40, Rgb([120, 120, 120]));
        let g = grid::build(&target, 10).unwrap();
        let tiles: Vec<Tile> = (0..30)
            .map(|i| {
                solid(
                    &format!("t{i}"),
                    [100 + i as u8, 100 + i as u8, 100 + i as u8],
                )
            })
            .collect();
        let a = assign(
            &g,
            &tiles,
            &MatchParams {
                exclusion_radius: 2,
                penalty_factor: 0.5,
                seed: 1,
            },
        );
        for c in &g.cells {
            let me = a.tile_of_cell[g.index(c.col, c.row)];
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nc, nr) = (c.col as i32 + dx, c.row as i32 + dy);
                    if nc < 0 || nr < 0 || nc >= 10 || nr >= 10 {
                        continue;
                    }
                    assert_ne!(me, a.tile_of_cell[g.index(nc as u32, nr as u32)]);
                }
            }
        }
    }

    #[test]
    fn is_deterministic_and_picks_obvious_colours() {
        let mut target = RgbImage::from_pixel(20, 10, Rgb([255, 0, 0]));
        for y in 0..10 {
            for x in 10..20 {
                target.put_pixel(x, y, Rgb([0, 0, 255]));
            }
        }
        let g = grid::build(&target, 2).unwrap();
        let tiles = vec![
            solid("red", [255, 0, 0]),
            solid("blue", [0, 0, 255]),
            solid("green", [0, 255, 0]),
        ];
        let p = MatchParams {
            exclusion_radius: 0,
            penalty_factor: 0.0,
            seed: 7,
        };
        let a = assign(&g, &tiles, &p);
        let b = assign(&g, &tiles, &p);
        assert_eq!(a.tile_of_cell, vec![0, 1]);
        assert_eq!(a.tile_of_cell, b.tile_of_cell);
    }
}
