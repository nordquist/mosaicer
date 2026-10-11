# Mosaicer

CLI that recreates a target image as a photo mosaic: the target is split into a grid, and every cell is replaced by the source image whose 3x3 colour signature matches that cell best, with a penalty against reusing the same tile and a ban on repeats within a 2-cell radius.

**Status:** v1 implemented end-to-end (index, match, render, quality report). Video input is detected but not yet used. Tuning values are hard-coded in `src/config.rs` and will become CLI flags.

![EXAMPLE](/example.png)


## Usage

```
mosaicer --target-image <FILE> [--source-images <DIR>]

  -t, --target-image <FILE>   Image to recreate (webp, png, jpg)
  -s, --source-images <DIR>   Folder with >= 10 source files (jpg, png, webp, heic, mp4).
                              Default: images/source (relative to cwd)
```

```sh
cargo run --release -- -t images/target.webp
cargo run --release -- -t photo.png -s ~/Pictures/holiday
```

Output is written next to the target as `<target-stem>-mosaic.png`. Example run on the bundled sample:

```
target:  images/target.webp
sources: images/source (218 images, 63 videos, 0 unsupported skipped)
index:   217 tiles (0 cached, 218 decoded, 1 duplicates dropped, 0 failed, 63 videos skipped: not yet supported)
         indexed in 6.2s
grid:    48x73 = 3504 cells, 217 tiles, reuse penalty λ=0.0472
match:   ΔE mean 0.291, p95 0.463, max 0.513; 87.2% of cells poorly matched (ΔE > 0.12)
usage:   max 25 per tile, median 16, 0 tiles unused; most used: 20221021_154206.jpg×25, ...
hint:    source library lacks colours present in the target: dark (1734 cells), yellow (508 cells), grey (338 cells)
         matched in 0.01s
output:  images/target-mosaic.png (3072x4672, blend 0.2)
```

Cold run 6.8 s on an M1 Pro (10 threads); warm run 0.15 s thanks to the index cache. The high "poorly matched" share on the sample is real: the sample library (selfies, azulejo tiles) has almost no dark tones and the target is a dark portrait. The hint line names the missing colours.

Validation rules:

- Target must exist and have extension `webp`, `png`, `jpg` or `jpeg` (case-insensitive).
- Source must be a directory with at least 10 files of type `jpg`/`jpeg`/`png`/`webp`/`heic`/`heif`/`mp4`. Scan is non-recursive, hidden files ignored, other extensions reported as skipped.
- `mp4` files count toward the minimum but are skipped during indexing for now (see Analysis: no pure-Rust HEVC decoder).
- `heic`/`heif` are decoded through macOS `sips`; on other platforms they fail to decode and are reported under `failed`.
- Exit code 1 with a one-line `error: ...` on invalid input or if no source could be decoded.

## How it works

1. **Index** (`index.rs`, parallel with rayon). Each still image is decoded with EXIF orientation applied, centre-cropped to a square, thumbnailed to `TILE_PX` (64) and described by a 3x3 grid of mean Oklab colours computed in linear light (27 floats). Results are cached in `<source-dir>/.mosaicer-index.json` keyed by file name, size and mtime; the cache is ignored if `TILE_PX` or the cache version changes. Near-identical tiles (rms ΔE < `DUP_EPS` 0.02) are dropped, first by name wins.
2. **Grid** (`grid.rs`). `GRID_COLS` (48) columns; rows follow the target aspect ratio (448x684 -> 48x73). Each cell gets the same 3x3 descriptor straight from the full-resolution target, plus its mean colour and a saliency score (variance of L over the 9 sub-cells).
3. **Match** (`matcher.rs`). Full distance matrix cells x tiles (brute force, ~0.01 s). Cells are processed most-salient first (edges and faces get first pick), ties broken by a ChaCha8 shuffle with fixed `SEED`. Cost = weighted squared Oklab distance (`LUMA_WEIGHT` 2 on L) + λ·uses[tile], λ = `PENALTY_FACTOR` (0.5) x median nearest-neighbour distance². A tile already placed within Chebyshev radius `EXCLUSION_RADIUS` (2) is forbidden; if every tile is forbidden the radius is ignored for that cell.
4. **Compose** (`compose.rs`). Tiles are pasted at `TILE_PX` and blended toward the cell mean colour with `BLEND_ALPHA` (0.2). Output is clamped to `MAX_OUTPUT_PX` (8192) per side.
5. **Report** (`report.rs`). Mean/p95/max raw ΔE, share of cells above `GAP_THRESHOLD` (0.12), tile usage, and hints when distinct tiles < 50 or > 20 % of cells are poorly matched, naming the dominant missing colours.

Determinism: sorted file list, fixed seed, index-order tie-breaking. Same inputs give the same output.

## Build

Requires Rust 1.84+ (`rust-version` in `Cargo.toml`, `resolver = "3"` keeps dependencies within that MSRV).

```sh
cargo build --release
cargo test
cargo clippy --all-targets
```

Dependencies: `image` 0.25 (jpeg/png/webp, EXIF orientation), `rayon`, `serde`/`serde_json`/`base64` (index cache), `rand`/`rand_chacha` (seeded order), `clap`, `anyhow`. No system libraries; HEIC uses the `sips` binary that ships with macOS.

## Project layout

```
src/
  main.rs        entry point, error -> exit code
  cli.rs         clap arguments and defaults
  config.rs      hard-coded tuning constants (future CLI flags)
  media.rs       MediaKind, extension classification, directory scan
  validate.rs    target / source validation rules
  decode.rs      image decoding + EXIF orientation; HEIC via sips on macOS
  color.rs       sRGB <-> linear <-> Oklab
  descriptor.rs  3x3 Oklab descriptor, distance, saliency
  index.rs       source indexing, square crop, thumbnails, dedupe, JSON cache
  grid.rs        target grid and per-cell descriptors
  matcher.rs     greedy saliency-first assignment with reuse penalty
  compose.rs     output rendering and blending
  report.rs      quality statistics and user hints
  mosaic.rs      pipeline orchestration
images/
  target.webp    sample target (448x684)
  source/        sample source library
```

## Roadmap

- CLI flags for `GRID_COLS`, `TILE_PX`, `BLEND_ALPHA`, `EXCLUSION_RADIUS`, `SEED`, output path.
- Video first frames: `ffmpeg` on `PATH` when present, macOS `qlmanage` fallback otherwise (see Analysis).
- Optional exact assignment mode.

## Analysis

### Why the naive approach ("mean colour per cell, pick nearest") is not enough

- **Mean colour loses structure.** A cell that is half sky / half roof averages to grey; the chosen tile then has the right average and no edge. Edges and faces vanish. Fix: describe cells and tiles by a 3x3 grid of sub-means (27 values), not one colour.
- **Independent nearest neighbour repeats tiles.** Flat regions (sky, wall, skin) map to the same tile dozens of times in a row. At 48 columns the sample target has ~3,500 cells against ~150 usable tiles: mean reuse ~23 per tile, concentrated in flat areas. Fix: reuse penalty + local exclusion radius (below).
- **Library gaps.** If no tile is near saturated red, every red cell gets the least-bad beige. No matching algorithm can fix a missing colour; only (a) tinting/blending the tile toward the cell colour, or (b) more varied sources. Mosaicer must measure the gap and tell the user.
- **Near-duplicates inflate the library count without adding coverage.** Every one of the 63 sample MP4s is an iPhone Live Photo companion of a still in the same folder (57 HEIC + 6 JPG, same stem). Their first frames add zero colour coverage. Fix: dedupe by descriptor distance after indexing.
- **Aspect ratio.** Sources are 3:4 / 4:3 / 1:1 mixed; cells are fixed-aspect. Tiles must be centre-cropped to the cell aspect before describing them, or the descriptor describes pixels that never get shown.
- **EXIF orientation.** iPhone portrait JPGs are stored landscape + orientation tag. `image` does not auto-rotate; without rotation the 3x3 descriptor is wrong. Fix: read EXIF (kamadak-exif) and rotate before describing.
- **Colour profiles.** Some sample JPGs are Display P3, others sRGB. `image` ignores ICC profiles, so P3 tiles come out slightly desaturated. Acceptable for a mosaic; noted, not fixed in v1.
- **Averaging in sRGB.** Mean of gamma-encoded values is darker than the true mean. Small effect at 3x3 sub-cells; v1 averages in linear light anyway since it is cheap.

### Proposed algorithm

Descriptor `D(img)` for any image (cell or tile), after crop to cell aspect:
1. Convert sRGB bytes to linear RGB.
2. Box-average into a 3x3 grid (9 mean linear-RGB values).
3. Convert each to Oklab → 27 floats `(L,a,b) x 9`.

Distance: `dist²(x, y) = Σ_i [ wL·(Lx−Ly)² + (ax−ay)² + (bx−by)² ]` with `wL = 2` (luminance carries structure).

Pipeline:

1. **Index sources** (parallel, rayon). For each supported file: decode (mp4 → first frame), apply EXIF rotation, centre-crop to cell aspect, downscale to tile thumbnail (`TILE_PX`, default 64), compute `D`. Persist to `.mosaicer-index.json` in the source folder keyed by `(path, size, mtime, cell_aspect, TILE_PX)` so re-runs skip decoding. Drop tiles whose `dist` to an already-kept tile < `DUP_EPS` (Live Photo pairs, bursts). Report: files found / skipped / decode failures / duplicates dropped / distinct tiles.
2. **Target grid.** `GRID_COLS` hard-coded (default 48; CLI flag later). Square cells: `cell_px = target_w / GRID_COLS`, `rows = round(target_h / cell_px)`. For 448x684 → 48x73 = 3,504 cells. Compute `D` per cell straight from the full-res target (no intermediate downscale needed; the 3x3 box average is the downscale).
3. **Assign** (sequential greedy with penalty, brute force; 3,500 cells x 150 tiles x 27 dims is ~14 M flops, no spatial index needed):
   ```
   order = cells sorted by saliency desc        # saliency = variance of the 9 L values
           ties broken by ChaCha8(seed) shuffle # deterministic
   uses[tile] = 0
   for cell in order:
       forbidden = tiles already placed within Chebyshev radius R of cell   # R = 2 → ≤ 24 tiles
       best = argmin over tile ∉ forbidden of  dist²(D cell, D tile) + λ · uses[tile]
       place(cell, best); uses[best] += 1
   λ = 0.5 · median over cells of (raw nearest-neighbour dist²)      # scale-free
   ```
   Saliency-first ordering gives edges and faces first pick; smooth regions get leftovers where errors are least visible. Raster order would produce a top-to-bottom quality gradient.
4. **Compose.** Output `(GRID_COLS·TILE_PX) x (rows·TILE_PX)` RGB8 (48x73 at 64 px = 3072x4672, ~43 MB in memory). Paste thumbnails. Optional blend `out = (1−α)·tile + α·cell_mean` with `α` default 0 (flag later; above ~0.3 it stops looking like a mosaic). Write PNG next to the target as `<target>-mosaic.png`.
5. **Gap report.** Print mean/95th-percentile raw distance, % of cells whose nearest tile exceeds `GAP_THRESHOLD`, tile usage histogram (max / median). Nudge the user when distinct tiles < 50 or > 20 % of cells are poorly matched, with the dominant unmatched hues named (e.g. "no sources near: saturated red, deep blue").

Global optimum assignment (Hungarian / min-cost flow with k copies per tile) was considered and rejected for v1: O(n³) at n≈3,500 is seconds-to-minutes, it cannot express "no repeat within radius" (that is a quadratic assignment problem), and minimising the summed error spreads forced bad matches across the whole image instead of hiding them. Possible later `--assign exact`.

### Pitfalls and mitigations

| Pitfall | Impact | Mitigation |
|---|---|---|
| Too few distinct tiles (sample: ~150) | visible repetition | reuse penalty, exclusion radius, dedupe report, nudge user |
| Colour gaps in library | wrong hues in whole regions | gap report; optional blend α; nudge user |
| Live Photo / burst duplicates | fake diversity | descriptor-distance dedupe after indexing |
| EXIF orientation ignored | rotated tiles, wrong descriptors | kamadak-exif + rotate before describing |
| Tile aspect ≠ cell aspect | descriptor describes cropped-away pixels | centre-crop to cell aspect first |
| Decoding 100+ x 7-12 MP photos every run | tens of seconds | sidecar index cache keyed by path+size+mtime; rayon |
| Memory for full-res decode in parallel | 10 threads x 12 MP x 3 B = ~360 MB peak | fine on 16 GB; cap threads if needed |
| HEVC Live Photo videos | no pure-Rust decoder | see decoding section |
| Non-deterministic output | un-reproducible results | fixed seed, sorted file list, index tie-break |
| Grid too fine for TILE_PX | huge output image | clamp `cols·TILE_PX` (e.g. ≤ 8192 px) |
| P3 vs sRGB sources | slight saturation mismatch | accept for v1; document |

### Input decoding on this machine

| Input | Status | Crate / tool |
|---|---|---|
| jpg, png, webp (lossy VP8 like `target.webp` and lossless) | pure Rust | `image` 0.25 (cargo resolves 0.25.6 under rust-version 1.84), `image-webp` |
| EXIF orientation | pure Rust | `kamadak-exif` 0.6.1 (MSRV 1.60) |
| Fast downscale | optional | `fast_image_resize` 5.1.4 (6.x needs rustc 1.87); `image::imageops::resize` is enough for v1 |
| Parallelism / cache / RNG | pure Rust | `rayon` 1.12, `serde_json` 1.0, `rand_chacha` 0.9 |
| MP4 H.264 (`avc1`) | pure Rust possible | `mp4` 0.14 demux + `openh264` 0.8.1 (MSRV 1.83); covers 6 of 63 sample files |
| MP4 HEVC (`hvc1`) | **no pure-Rust decoder** | 57 of 63 sample files. Options: `ffmpeg` binary via `ffmpeg-sidecar` 2.6.0 (MSRV 1.79; can auto-download ffmpeg), or macOS `qlmanage -t` poster frame (verified works, no install) |
| HEIC | out of spec; **no pure-Rust decoder** | `libheif-rs` 3.0 needs system `libheif` (not installed, `brew install libheif`); macOS `sips -s format jpeg` works today (verified) |

`ffmpeg`/`ffprobe` are not installed here. Recommended strategy: video frames via `ffmpeg` on `PATH` when present, else macOS `qlmanage` fallback, else skip the file with a warning. All three paths behind one `FrameExtractor` trait so the core never cares.

### Dataset facts (`images/source`)

| Fact | Value |
|---|---|
| Files | 281 (100 JPG, 118 HEIC, 63 MP4), 687 MB |
| Distinct stems | 218 = exactly the stills; every MP4 shares a stem with a still |
| MP4 codecs | 57 HEVC, 6 H.264 |
| Typical dimensions | 2316x3088 / 3088x2316 (7 MP), 2944x2208 |
| Colour profiles | mix of sRGB and Display P3 |
| Target | 448x684 lossy VP8 webp |
| Usable tiles under current spec (no HEIC) | 100 JPG + 57 MP4 frames that duplicate unsupported HEIC = ~157 distinct; 6 MP4 frames duplicate JPGs |
| Hardware | Apple M1 Pro, 10 cores, 16 GB |

### Decisions taken (2026-10-11)

1. HEIC: supported via macOS `sips` (zero extra dependencies); skipped with a decode error elsewhere.
2. Videos: skipped for now. 57 of 63 sample MP4s are HEVC and every one duplicates a still, so nothing is lost on the sample set.
3. Cells: square, centre-cropped tiles.
4. Blending: on by default, `BLEND_ALPHA` 0.2.
5. Output: PNG next to the target, `TILE_PX` 64, `GRID_COLS` 48.
6. Dedupe: by descriptor distance only (`DUP_EPS` 0.02). Stem-based Live Photo dedupe becomes relevant once videos are indexed.
