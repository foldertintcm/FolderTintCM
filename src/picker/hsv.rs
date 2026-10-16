//! Hue, saturation and value: the picker's own coordinates. The square is
//! saturation across and value down; the bar below it is the hue.

use crate::palette::Rgb;

/// Each component 0–1. The hue is kept even where it means nothing (grey,
/// black), so dragging to a corner and back does not lose it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    pub fn to_rgb(self) -> Rgb {
        let [r, g, b] = rgb_f(self.h, self.s, self.v);
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgb([byte(r), byte(g), byte(b)])
    }

    /// The colour as HSV, keeping `hue` when the colour has none.
    pub fn from_rgb(rgb: Rgb, hue: f32) -> Hsv {
        let [r, g, b] = rgb.0.map(|c| c as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let s = if max > 0.0 { d / max } else { 0.0 };
        let h = if d <= f32::EPSILON {
            hue
        } else if max == r {
            ((g - b) / d).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        Hsv { h, s, v: max }
    }
}

/// HSV to linear-free sRGB floats, 0–1.
pub fn rgb_f(h: f32, s: f32, v: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h.rem_euclid(1.0) * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hsv(h: f32, s: f32, v: f32) -> Rgb {
        Hsv { h, s, v }.to_rgb()
    }

    #[test]
    fn primaries_and_greys() {
        assert_eq!(hsv(0.0, 1.0, 1.0), Rgb([255, 0, 0]));
        assert_eq!(hsv(1.0 / 3.0, 1.0, 1.0), Rgb([0, 255, 0]));
        assert_eq!(hsv(2.0 / 3.0, 1.0, 1.0), Rgb([0, 0, 255]));
        assert_eq!(hsv(0.3, 0.0, 0.5), Rgb([128, 128, 128]));
        assert_eq!(hsv(0.3, 1.0, 0.0), Rgb([0, 0, 0]));
    }

    #[test]
    fn every_byte_colour_round_trips() {
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(51) {
                    let c = Rgb([r as u8, g as u8, b as u8]);
                    assert_eq!(Hsv::from_rgb(c, 0.0).to_rgb(), c, "{c:?}");
                }
            }
        }
    }

    #[test]
    fn greys_keep_the_hue_they_had() {
        let grey = Hsv::from_rgb(Rgb([90, 90, 90]), 0.42);
        assert_eq!(grey.h, 0.42);
        assert_eq!(grey.s, 0.0);
    }
}
