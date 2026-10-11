use crate::color::Oklab;
use crate::config::{GAP_NUDGE_FRACTION, GAP_THRESHOLD, MIN_DISTINCT_TILES};
use crate::grid::Grid;
use crate::index::{IndexStats, Tile};
use crate::matcher::Assignment;

pub fn print_index(stats: &IndexStats, tiles: &[Tile]) {
    println!(
        "index:   {} tiles ({} cached, {} decoded, {} duplicates dropped, {} failed, {} videos skipped: not yet supported)",
        tiles.len(), stats.from_cache, stats.decoded, stats.duplicates_dropped, stats.failed.len(), stats.videos_skipped
    );
    for (name, err) in &stats.failed {
        println!("  ! {name}: {err}");
    }
}

pub fn print_quality(grid: &Grid, tiles: &[Tile], a: &Assignment) {
    let mut d = a.raw_nn_dist.clone();
    d.sort_by(|x, y| x.total_cmp(y));
    let n = d.len();
    let mean = d.iter().sum::<f32>() / n as f32;
    let p95 = d[(n as f32 * 0.95) as usize];
    let poor: Vec<usize> = (0..n)
        .filter(|&i| a.raw_nn_dist[i] > GAP_THRESHOLD)
        .collect();
    let poor_frac = poor.len() as f32 / n as f32;

    let mut uses = a.uses.clone();
    uses.sort_unstable();
    let unused = uses.iter().filter(|&&u| u == 0).count();

    println!(
        "grid:    {}x{} = {} cells, {} tiles, reuse penalty λ={:.4}",
        grid.cols,
        grid.rows,
        n,
        tiles.len(),
        a.lambda
    );
    println!(
        "match:   ΔE mean {:.3}, p95 {:.3}, max {:.3}; {:.1}% of cells poorly matched (ΔE > {GAP_THRESHOLD})",
        mean, p95, d[n - 1], poor_frac * 100.0
    );
    let mut top: Vec<(u32, &str)> = a
        .uses
        .iter()
        .enumerate()
        .map(|(i, &u)| (u, tiles[i].name.as_str()))
        .collect();
    top.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(y.1)));
    let top: Vec<String> = top
        .iter()
        .take(3)
        .map(|(u, n)| format!("{n}×{u}"))
        .collect();
    println!(
        "usage:   max {} per tile, median {}, {} tiles unused; most used: {}",
        uses[uses.len() - 1],
        uses[uses.len() / 2],
        unused,
        top.join(", ")
    );

    if tiles.len() < MIN_DISTINCT_TILES {
        println!("hint:    only {} distinct tiles; add more varied source images (aim for ≥ {MIN_DISTINCT_TILES})", tiles.len());
    }
    if poor_frac > GAP_NUDGE_FRACTION {
        let names = dominant_colours(poor.iter().map(|&i| grid.cells[i].mean));
        println!(
            "hint:    source library lacks colours present in the target: {}",
            names.join(", ")
        );
    }
}

fn dominant_colours(cells: impl Iterator<Item = Oklab>) -> Vec<String> {
    const HUES: [(&str, f32); 6] = [
        ("red", 30.0),
        ("yellow", 100.0),
        ("green", 140.0),
        ("cyan", 200.0),
        ("blue", 260.0),
        ("magenta", 330.0),
    ];
    let mut counts: std::collections::HashMap<&str, usize> = Default::default();
    for c in cells {
        let chroma = (c.a * c.a + c.b * c.b).sqrt();
        let name = if c.l < 0.25 {
            "dark"
        } else if c.l > 0.9 {
            "very light"
        } else if chroma < 0.04 {
            "grey"
        } else {
            let h = c.b.atan2(c.a).to_degrees().rem_euclid(360.0);
            HUES.iter()
                .min_by(|x, y| ang(h, x.1).total_cmp(&ang(h, y.1)))
                .map(|x| x.0)
                .unwrap_or("colour")
        };
        *counts.entry(name).or_default() += 1;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.into_iter()
        .take(3)
        .map(|(n, k)| format!("{n} ({k} cells)"))
        .collect()
}

fn ang(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}
