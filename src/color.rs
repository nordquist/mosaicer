use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    let mut lut = [0f32; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
});

#[inline]
pub fn srgb_to_linear(c: u8) -> f32 {
    SRGB_TO_LINEAR[c as usize]
}

pub fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let c = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

pub fn linear_to_oklab(c: LinearRgb) -> Oklab {
    let l = (0.412_221_47 * c.r + 0.536_332_54 * c.g + 0.051_445_995 * c.b).cbrt();
    let m = (0.211_903_5 * c.r + 0.680_699_5 * c.g + 0.107_396_96 * c.b).cbrt();
    let s = (0.088_302_46 * c.r + 0.281_718_85 * c.g + 0.629_978_7 * c.b).cbrt();
    Oklab {
        l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    }
}

pub fn oklab_to_linear(c: Oklab) -> LinearRgb {
    let l = (c.l + 0.396_337_78 * c.a + 0.215_803_76 * c.b).powi(3);
    let m = (c.l - 0.105_561_346 * c.a - 0.063_854_17 * c.b).powi(3);
    let s = (c.l - 0.089_484_18 * c.a - 1.291_485_5 * c.b).powi(3);
    LinearRgb {
        r: 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_93 * s,
        g: -1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s,
        b: -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    }
}

pub fn oklab_to_srgb(c: Oklab) -> [u8; 3] {
    let lin = oklab_to_linear(c);
    [
        linear_to_srgb(lin.r),
        linear_to_srgb(lin.g),
        linear_to_srgb(lin.b),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_roundtrip_and_known_values() {
        let white = linear_to_oklab(LinearRgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        });
        assert!((white.l - 1.0).abs() < 1e-3 && white.a.abs() < 1e-3 && white.b.abs() < 1e-3);
        for rgb in [[255u8, 0, 0], [12, 200, 77], [0, 0, 0], [128, 128, 128]] {
            let lin = LinearRgb {
                r: srgb_to_linear(rgb[0]),
                g: srgb_to_linear(rgb[1]),
                b: srgb_to_linear(rgb[2]),
            };
            let back = oklab_to_srgb(linear_to_oklab(lin));
            for i in 0..3 {
                assert!(
                    (back[i] as i32 - rgb[i] as i32).abs() <= 1,
                    "{rgb:?} -> {back:?}"
                );
            }
        }
    }
}
