//! The folder, drawn: a back panel with its tab and a front panel over it,
//! in the style of macOS' folders, from signed distance fields so every size
//! from 16 to 256 pixels is drawn crisp rather than scaled.
//!
//! Pure Rust, no Windows calls: `build.rs` includes this file too, to draw
//! the executable's own icon.

/// The drawing is laid out on a 256-unit square.
const UNITS: f32 = 256.0;

/// A rounded rectangle, centre and half-extents in units.
struct RoundBox {
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    r: f32,
}

impl RoundBox {
    /// Signed distance, in units, from (x, y); negative inside.
    fn sdf(&self, x: f32, y: f32) -> f32 {
        let qx = (x - self.cx).abs() - (self.hw - self.r);
        let qy = (y - self.cy).abs() - (self.hh - self.r);
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - self.r
    }

    fn top(&self) -> f32 {
        self.cy - self.hh
    }

    fn bottom(&self) -> f32 {
        self.cy + self.hh
    }
}

const BACK: RoundBox = RoundBox {
    cx: 128.0,
    cy: 134.0,
    hw: 110.0,
    hh: 80.0,
    r: 18.0,
};
const TAB: RoundBox = RoundBox {
    cx: 68.0,
    cy: 50.0,
    hw: 50.0,
    hh: 14.0,
    r: 12.0,
};
const FRONT: RoundBox = RoundBox {
    cx: 128.0,
    cy: 150.0,
    hw: 110.0,
    hh: 64.0,
    r: 18.0,
};

/// Polynomial smooth minimum: the tab flows into the back panel with a
/// fillet instead of a notch.
fn smooth_union(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

/// Area covered by a shape at signed distance `d` pixels, over a soft edge
/// `width` pixels wide (1 is a plain anti-aliased edge).
fn coverage(d: f32, width: f32) -> f32 {
    (0.5 - d / width.max(1.0)).clamp(0.0, 1.0)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const BLACK: [f32; 3] = [0.0, 0.0, 0.0];

/// Premultiplied RGBA, painted bottom layer first.
#[derive(Clone, Copy, Default)]
struct Pixel([f32; 4]);

impl Pixel {
    fn over(&mut self, rgb: [f32; 3], alpha: f32) {
        let keep = 1.0 - alpha;
        for (dst, src) in self.0.iter_mut().zip(rgb) {
            *dst = src * alpha + *dst * keep;
        }
        self.0[3] = alpha + self.0[3] * keep;
    }

    /// Straight-alpha RGBA bytes.
    fn to_rgba(self) -> [u8; 4] {
        let a = self.0[3];
        if a <= 0.0 {
            return [0; 4];
        }
        let c = |v: f32| ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8;
        [
            c(self.0[0]),
            c(self.0[1]),
            c(self.0[2]),
            (a * 255.0).round() as u8,
        ]
    }
}

/// The folder at `size` × `size` pixels, straight-alpha RGBA, top-down.
/// `colour(u)` gives the folder's base colour at `u`, 0 on the left edge of
/// the folder to 1 on the right: one colour for a tint, a spectrum for the
/// application's own icon.
pub fn folder(size: u32, colour: impl Fn(f32) -> [f32; 3]) -> Vec<u8> {
    let k = size as f32 / UNITS; // pixels per unit
    let left = BACK.cx - BACK.hw;
    let width = BACK.hw * 2.0;
    // Shadows only where there are pixels enough to show them: at 16 px
    // they only muddy the outline.
    let shadows = size >= 32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let x = (px as f32 + 0.5) / k;
            let y = (py as f32 + 0.5) / k;
            let base = colour(((x - left) / width).clamp(0.0, 1.0));
            let back_d = smooth_union(BACK.sdf(x, y), TAB.sdf(x, y), 8.0);
            let front_d = FRONT.sdf(x, y);
            let mut p = Pixel::default();

            if shadows {
                // Soft contact shadow under the whole folder.
                let drop = smooth_union(BACK.sdf(x, y - 4.0), TAB.sdf(x, y - 4.0), 8.0);
                p.over(BLACK, 0.22 * coverage(drop * k, 10.0 * k));
            }

            // Back panel: the tint, a shade darker.
            p.over(mix(base, BLACK, 0.18), coverage(back_d * k, 1.0));

            if shadows {
                // The front panel's edge casts onto the back panel.
                let lip = FRONT.sdf(x, y + 3.0);
                p.over(
                    BLACK,
                    0.18 * coverage(lip * k, 6.0 * k) * coverage(back_d * k, 1.0),
                );
            }

            // Front panel: lighter at the top, the plain tint at the bottom,
            // with a bright lip along its top edge.
            let v = ((y - FRONT.top()) / (FRONT.bottom() - FRONT.top())).clamp(0.0, 1.0);
            let mut front = mix(base, WHITE, 0.20 * (1.0 - v));
            let lip = (1.0 - (y - FRONT.top()) / (3.0_f32).max(1.2 / k)).clamp(0.0, 1.0);
            front = mix(front, WHITE, 0.35 * lip);
            p.over(front, coverage(front_d * k, 1.0));

            out.extend_from_slice(&p.to_rgba());
        }
    }
    out
}

/// sRGB bytes to the 0–1 floats `folder` mixes in.
pub fn unit_rgb(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| c as f32 / 255.0)
}

/// The application's mark: the folder painted across the spectrum. Drawn
/// by `build.rs` into the executable's icon.
#[allow(dead_code)]
pub fn spectrum(size: u32) -> Vec<u8> {
    folder(size, |u| hsv(u * 0.83, 0.78, 1.0))
}

#[allow(dead_code)]
fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn corners_are_transparent_and_body_is_opaque() {
        for size in [16, 32, 48, 256] {
            let px = folder(size, |_| unit_rgb([255, 59, 48]));
            assert_eq!(px.len(), (size * size * 4) as usize);
            assert_eq!(at(&px, size, 0, 0)[3], 0, "{size}: top-left");
            assert_eq!(at(&px, size, size - 1, 0)[3], 0, "{size}: top-right");
            assert_eq!(
                at(&px, size, size / 2, size * 3 / 5)[3],
                255,
                "{size}: body"
            );
        }
    }

    #[test]
    fn front_panel_carries_the_tint() {
        let size = 64;
        let px = folder(size, |_| unit_rgb([0, 122, 255]));
        let [r, g, b, _] = at(&px, size, size / 2, size * 3 / 4);
        assert!(b > 200 && b > g && g > r, "blue dominates: {r} {g} {b}");
    }

    #[test]
    fn tab_sits_on_the_left_only() {
        let size = 256;
        let px = folder(size, |_| unit_rgb([0, 0, 0]));
        assert_eq!(at(&px, size, 60, 44)[3], 255, "tab on the left");
        assert_eq!(
            at(&px, size, 200, 44)[3],
            0,
            "nothing on the right at tab height"
        );
    }

    #[test]
    fn spectrum_runs_from_red_to_violet() {
        let size = 64;
        let px = spectrum(size);
        let left = at(&px, size, 8, 48);
        let right = at(&px, size, 56, 48);
        assert!(left[0] > left[2], "red on the left");
        assert!(right[2] > right[1], "blue-violet on the right");
    }
}
