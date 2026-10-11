use image::RgbImage;
use serde::{Deserialize, Serialize};

use crate::color::{linear_to_oklab, srgb_to_linear, LinearRgb, Oklab};
use crate::config::LUMA_WEIGHT;

pub const GRID: usize = 3;
pub const CELLS: usize = GRID * GRID;
pub const DIM: usize = CELLS * 3;

/// 3x3 grid of mean Oklab colours (L,a,b per sub-cell), computed in linear light.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Descriptor(pub [f32; DIM]);

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn full(img: &RgbImage) -> Self {
        Rect {
            x: 0,
            y: 0,
            w: img.width(),
            h: img.height(),
        }
    }
}

pub fn describe(img: &RgbImage, rect: Rect) -> Descriptor {
    let mut out = [0f32; DIM];
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (x0, x1) = span(rect.x, rect.w, gx, img.width());
            let (y0, y1) = span(rect.y, rect.h, gy, img.height());
            let mut acc = LinearRgb::default();
            let mut n = 0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = img.get_pixel(x, y).0;
                    acc.r += srgb_to_linear(p[0]);
                    acc.g += srgb_to_linear(p[1]);
                    acc.b += srgb_to_linear(p[2]);
                    n += 1.0;
                }
            }
            let n = n.max(1.0);
            let lab = linear_to_oklab(LinearRgb {
                r: acc.r / n,
                g: acc.g / n,
                b: acc.b / n,
            });
            let i = (gy * GRID + gx) * 3;
            out[i] = lab.l;
            out[i + 1] = lab.a;
            out[i + 2] = lab.b;
        }
    }
    Descriptor(out)
}

/// Sub-span `k` of `GRID` across [start, start+len), at least 1 px wide, clamped to the image.
fn span(start: u32, len: u32, k: usize, limit: u32) -> (u32, u32) {
    let a = start + (len as u64 * k as u64 / GRID as u64) as u32;
    let b = start + (len as u64 * (k as u64 + 1) / GRID as u64) as u32;
    let a = a.min(limit.saturating_sub(1));
    let b = b.max(a + 1).min(limit);
    (a, b)
}

/// Mean over sub-cells of weighted squared Oklab difference; sqrt of this is comparable to ΔE_ok.
pub fn distance_sq(a: &Descriptor, b: &Descriptor) -> f32 {
    let mut sum = 0f32;
    for i in 0..CELLS {
        let j = i * 3;
        let dl = a.0[j] - b.0[j];
        let da = a.0[j + 1] - b.0[j + 1];
        let db = a.0[j + 2] - b.0[j + 2];
        sum += LUMA_WEIGHT * dl * dl + da * da + db * db;
    }
    sum / CELLS as f32
}

/// Variance of L across sub-cells: high for edges/texture, zero for flat colour.
pub fn saliency(d: &Descriptor) -> f32 {
    let mean = (0..CELLS).map(|i| d.0[i * 3]).sum::<f32>() / CELLS as f32;
    (0..CELLS).map(|i| (d.0[i * 3] - mean).powi(2)).sum::<f32>() / CELLS as f32
}

pub fn mean_oklab(d: &Descriptor) -> Oklab {
    let mut m = Oklab::default();
    for i in 0..CELLS {
        m.l += d.0[i * 3];
        m.a += d.0[i * 3 + 1];
        m.b += d.0[i * 3 + 2];
    }
    Oklab {
        l: m.l / CELLS as f32,
        a: m.a / CELLS as f32,
        b: m.b / CELLS as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn flat_image_has_zero_saliency_and_self_distance() {
        let img = RgbImage::from_pixel(9, 9, Rgb([100, 150, 200]));
        let d = describe(&img, Rect::full(&img));
        assert_eq!(distance_sq(&d, &d), 0.0);
        assert!(saliency(&d) < 1e-9);
    }

    #[test]
    fn half_black_half_white_is_salient_and_differs_from_grey() {
        let mut img = RgbImage::from_pixel(30, 30, Rgb([0, 0, 0]));
        for y in 0..30 {
            for x in 15..30 {
                img.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }
        let split = describe(&img, Rect::full(&img));
        let grey = describe(
            &RgbImage::from_pixel(3, 3, Rgb([188, 188, 188])),
            Rect {
                x: 0,
                y: 0,
                w: 3,
                h: 3,
            },
        );
        assert!(saliency(&split) > 0.1);
        assert!(distance_sq(&split, &grey) > 0.1);
        let m = mean_oklab(&split);
        assert!((m.l - 0.5).abs() < 0.2);
    }

    #[test]
    fn tiny_rect_still_describes() {
        let img = RgbImage::from_pixel(2, 2, Rgb([10, 20, 30]));
        let d = describe(
            &img,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
        );
        assert!(d.0.iter().all(|v| v.is_finite()));
    }
}
