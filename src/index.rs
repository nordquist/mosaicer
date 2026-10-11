use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use base64::prelude::*;
use image::{imageops, RgbImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{DUP_EPS, INDEX_FILE, TILE_PX};
use crate::decode;
use crate::descriptor::{self, Descriptor, Rect};
use crate::media::{MediaFile, ScanResult};

pub struct Tile {
    pub name: String,
    pub descriptor: Descriptor,
    pub thumb: RgbImage,
}

#[derive(Default)]
pub struct IndexStats {
    pub from_cache: usize,
    pub decoded: usize,
    pub failed: Vec<(String, String)>,
    pub videos_skipped: usize,
    pub duplicates_dropped: usize,
}

const CACHE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    tile_px: u32,
    entries: Vec<CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    name: String,
    size: u64,
    mtime: u64,
    descriptor: Descriptor,
    thumb_b64: String,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key {
    name: String,
    size: u64,
    mtime: u64,
}

fn key_of(file: &MediaFile) -> Result<Key> {
    let meta = fs::metadata(&file.path)?;
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = file
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(Key {
        name,
        size: meta.len(),
        mtime,
    })
}

/// Index all still images in `scan`: decode, square-crop, thumbnail, describe. Cached on disk.
pub fn build(scan: &ScanResult, dir: &Path) -> Result<(Vec<Tile>, IndexStats)> {
    let mut stats = IndexStats {
        videos_skipped: scan.videos().count(),
        ..Default::default()
    };
    let cache_path = dir.join(INDEX_FILE);
    let mut cache = load_cache(&cache_path);

    let keyed: Vec<(Key, &MediaFile)> = scan
        .images()
        .filter_map(|f| key_of(f).ok().map(|k| (k, f)))
        .collect();

    let mut hits: Vec<(Key, Tile)> = Vec::new();
    let mut misses: Vec<(Key, &MediaFile)> = Vec::new();
    for (key, file) in keyed {
        match cache.remove(&key).and_then(|e| tile_from_cache(&key, e)) {
            Some(tile) => hits.push((key, tile)),
            None => misses.push((key, file)),
        }
    }
    stats.from_cache = hits.len();

    let decoded: Vec<(Key, Result<Tile>)> = misses
        .into_par_iter()
        .map(|(key, file)| {
            let tile = decode::decode(&file.path).map(|img| make_tile(key.name.clone(), &img));
            (key, tile)
        })
        .collect();

    let mut tiles: Vec<(Key, Tile)> = hits;
    for (key, result) in decoded {
        match result {
            Ok(tile) => {
                stats.decoded += 1;
                tiles.push((key, tile));
            }
            Err(e) => stats.failed.push((key.name, format!("{e:#}"))),
        }
    }
    tiles.sort_by(|a, b| a.0.name.cmp(&b.0.name));

    if let Err(e) = save_cache(&cache_path, &tiles) {
        eprintln!(
            "warning: could not write index cache {}: {e:#}",
            cache_path.display()
        );
    }

    let (kept, dropped) = dedupe(tiles.into_iter().map(|(_, t)| t).collect());
    stats.duplicates_dropped = dropped;
    Ok((kept, stats))
}

pub fn make_tile(name: String, img: &RgbImage) -> Tile {
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    let thumb = imageops::thumbnail(
        &*imageops::crop_imm(img, x, y, side, side),
        TILE_PX,
        TILE_PX,
    );
    let descriptor = descriptor::describe(&thumb, Rect::full(&thumb));
    Tile {
        name,
        descriptor,
        thumb,
    }
}

/// Keep first of any group of near-identical tiles (sorted by name, so deterministic).
fn dedupe(tiles: Vec<Tile>) -> (Vec<Tile>, usize) {
    let eps_sq = DUP_EPS * DUP_EPS;
    let mut kept: Vec<Tile> = Vec::with_capacity(tiles.len());
    let mut dropped = 0;
    for t in tiles {
        let dup = kept
            .iter()
            .any(|k| descriptor::distance_sq(&k.descriptor, &t.descriptor) < eps_sq);
        if dup {
            dropped += 1;
        } else {
            kept.push(t);
        }
    }
    (kept, dropped)
}

fn load_cache(path: &Path) -> HashMap<Key, CacheEntry> {
    let Ok(bytes) = fs::read(path) else {
        return HashMap::new();
    };
    let Ok(file) = serde_json::from_slice::<CacheFile>(&bytes) else {
        return HashMap::new();
    };
    if file.version != CACHE_VERSION || file.tile_px != TILE_PX {
        return HashMap::new();
    }
    file.entries
        .into_iter()
        .map(|e| {
            (
                Key {
                    name: e.name.clone(),
                    size: e.size,
                    mtime: e.mtime,
                },
                e,
            )
        })
        .collect()
}

fn tile_from_cache(key: &Key, e: CacheEntry) -> Option<Tile> {
    let raw = BASE64_STANDARD.decode(e.thumb_b64).ok()?;
    let thumb = RgbImage::from_raw(TILE_PX, TILE_PX, raw)?;
    Some(Tile {
        name: key.name.clone(),
        descriptor: e.descriptor,
        thumb,
    })
}

fn save_cache(path: &Path, tiles: &[(Key, Tile)]) -> Result<()> {
    let entries = tiles
        .iter()
        .map(|(k, t)| CacheEntry {
            name: k.name.clone(),
            size: k.size,
            mtime: k.mtime,
            descriptor: t.descriptor.clone(),
            thumb_b64: BASE64_STANDARD.encode(t.thumb.as_raw()),
        })
        .collect();
    let file = CacheFile {
        version: CACHE_VERSION,
        tile_px: TILE_PX,
        entries,
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(&file)?).context("write cache")?;
    fs::rename(&tmp, path).context("replace cache")?;
    Ok(())
}
