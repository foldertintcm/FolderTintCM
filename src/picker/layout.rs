//! Where everything sits, in device pixels for a given DPI scale, and
//! what is under the pointer. Laid out on a 540 × 488 DIP client area:
//!
//! ```text
//! ┌──────────┐ ┌──────────────────────────────┐
//! │ preview  │ │ saturation × value           │
//! │ (folder) │ │                              │
//! └──────────┘ │                              │
//! [ HEX field] └──────────────────────────────┘
//!  R G B        [ hue ─────────────────────── ]
//! Colores  ● ● ● ● ● ● ● ● ● ● ● ● ●
//! Recientes ● ● ● ● ● ●
//!                              [Cancelar] [Aplicar]
//! ```

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn dip(x: f32, y: f32, w: f32, h: f32, k: f32) -> Rect {
        Rect {
            x: x * k,
            y: y * k,
            w: w * k,
            h: h * k,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }

    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Shrunk by `d` on every side.
    pub fn inset(&self, d: f32) -> Rect {
        Rect {
            x: self.x + d,
            y: self.y + d,
            w: (self.w - 2.0 * d).max(0.0),
            h: (self.h - 2.0 * d).max(0.0),
        }
    }
}

pub const WIDTH_DIP: f32 = 540.0;
pub const HEIGHT_DIP: f32 = 488.0;
const PAD: f32 = 24.0;
const SWATCH: f32 = 28.0;
/// Room for 13 swatches across the content width.
const STEP: f32 = (WIDTH_DIP - 2.0 * PAD - SWATCH) / 12.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub scale: f32,
    pub width: i32,
    pub height: i32,
    pub preview: Rect,
    pub hex: Rect,
    pub rgb_text: Rect,
    pub sv: Rect,
    pub hue: Rect,
    pub palette_label: Rect,
    pub palette: Vec<Rect>,
    pub recent_label: Rect,
    pub recent: Vec<Rect>,
    /// Where the recent swatches would be: the hint when there are none.
    pub recent_row: Rect,
    pub cancel: Rect,
    pub apply: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Sv,
    Hue,
    Palette(usize),
    Recent(usize),
    Cancel,
    Apply,
    Nothing,
}

impl Layout {
    pub fn new(scale: f32, palette: usize, recent: usize) -> Layout {
        let k = scale;
        let row = |y: f32, n: usize| -> Vec<Rect> {
            (0..n)
                .map(|i| Rect::dip(PAD + i as f32 * STEP, y, SWATCH, SWATCH, k))
                .collect()
        };
        let right_x = PAD + 168.0 + 20.0;
        let right_w = WIDTH_DIP - PAD - right_x;
        Layout {
            scale,
            width: (WIDTH_DIP * k).round() as i32,
            height: (HEIGHT_DIP * k).round() as i32,
            preview: Rect::dip(PAD, PAD, 168.0, 168.0, k),
            hex: Rect::dip(PAD, 204.0, 168.0, 34.0, k),
            rgb_text: Rect::dip(PAD, 244.0, 168.0, 20.0, k),
            sv: Rect::dip(right_x, PAD, right_w, 188.0, k),
            hue: Rect::dip(right_x, 226.0, right_w, 16.0, k),
            palette_label: Rect::dip(PAD, 280.0, 300.0, 18.0, k),
            palette: row(304.0, palette),
            recent_label: Rect::dip(PAD, 348.0, 300.0, 18.0, k),
            recent: row(372.0, recent),
            recent_row: Rect::dip(PAD, 372.0, WIDTH_DIP - 2.0 * PAD, SWATCH, k),
            cancel: Rect::dip(
                WIDTH_DIP - PAD - 104.0 - 12.0 - 104.0,
                428.0,
                104.0,
                36.0,
                k,
            ),
            apply: Rect::dip(WIDTH_DIP - PAD - 104.0, 428.0, 104.0, 36.0, k),
        }
    }

    /// What a press at (x, y) acts on. The colour areas take a few pixels
    /// of slack so their edges (pure white, full hue) are easy to reach.
    pub fn hit(&self, x: f32, y: f32) -> Hit {
        let slack = 6.0 * self.scale;
        let round = |r: &Rect| {
            let (dx, dy) = (x - r.cx(), y - r.cy());
            (dx * dx + dy * dy).sqrt() <= r.w / 2.0 + 3.0 * self.scale
        };
        if let Some(i) = self.palette.iter().position(round) {
            return Hit::Palette(i);
        }
        if let Some(i) = self.recent.iter().position(round) {
            return Hit::Recent(i);
        }
        if self.cancel.contains(x, y) {
            return Hit::Cancel;
        }
        if self.apply.contains(x, y) {
            return Hit::Apply;
        }
        if self.sv.inset(-slack).contains(x, y) {
            return Hit::Sv;
        }
        if self.hue.inset(-slack).contains(x, y) {
            return Hit::Hue;
        }
        Hit::Nothing
    }

    /// Saturation and value at (x, y), clamped into the square.
    pub fn sv_at(&self, x: f32, y: f32) -> (f32, f32) {
        let s = ((x - self.sv.x) / self.sv.w).clamp(0.0, 1.0);
        let v = 1.0 - ((y - self.sv.y) / self.sv.h).clamp(0.0, 1.0);
        (s, v)
    }

    /// Hue at x, clamped to the bar.
    pub fn hue_at(&self, x: f32) -> f32 {
        ((x - self.hue.x) / self.hue.w).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fits_inside_the_window_at_every_scale() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let l = Layout::new(scale, 13, 6);
            let (w, h) = (l.width as f32, l.height as f32);
            let all = [
                l.preview,
                l.hex,
                l.rgb_text,
                l.sv,
                l.hue,
                l.recent_row,
                l.cancel,
                l.apply,
            ]
            .into_iter()
            .chain(l.palette.iter().copied())
            .chain(l.recent.iter().copied());
            for r in all {
                assert!(
                    r.x >= 0.0 && r.y >= 0.0 && r.right() <= w + 0.5 && r.bottom() <= h + 0.5,
                    "{scale}: {r:?}"
                );
            }
        }
    }

    #[test]
    fn hits_land_on_what_is_drawn() {
        let l = Layout::new(1.5, 13, 2);
        assert_eq!(l.hit(l.sv.cx(), l.sv.cy()), Hit::Sv);
        assert_eq!(l.hit(l.hue.cx(), l.hue.cy()), Hit::Hue);
        assert_eq!(
            l.hit(l.palette[12].cx(), l.palette[12].cy()),
            Hit::Palette(12)
        );
        assert_eq!(l.hit(l.recent[1].cx(), l.recent[1].cy()), Hit::Recent(1));
        assert_eq!(l.hit(l.cancel.cx(), l.cancel.cy()), Hit::Cancel);
        assert_eq!(l.hit(l.apply.cx(), l.apply.cy()), Hit::Apply);
        assert_eq!(l.hit(l.preview.cx(), l.preview.cy()), Hit::Nothing);
        // No third recent swatch to hit.
        let third = Layout::new(1.5, 13, 3).recent[2];
        assert_eq!(l.hit(third.cx(), third.cy()), Hit::Nothing);
    }

    #[test]
    fn square_and_bar_map_to_hsv() {
        let l = Layout::new(1.0, 13, 0);
        assert_eq!(l.sv_at(l.sv.x, l.sv.y), (0.0, 1.0));
        assert_eq!(
            l.sv_at(l.sv.right() + 50.0, l.sv.bottom() + 50.0),
            (1.0, 0.0)
        );
        assert_eq!(l.hue_at(l.hue.x - 10.0), 0.0);
        assert_eq!(l.hue_at(l.hue.right()), 1.0);
    }

    #[test]
    fn swatches_do_not_overlap() {
        let l = Layout::new(1.0, 13, 6);
        for pair in l.palette.windows(2) {
            assert!(pair[0].right() < pair[1].x);
        }
    }
}
