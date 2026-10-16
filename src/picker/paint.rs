//! The picker, painted into a pixel buffer: anti-aliased shapes from
//! signed distances, as the folder itself is drawn. The window then puts
//! the text on top with GDI and copies the buffer to the screen.

use super::hsv::{Hsv, rgb_f};
use super::layout::{Hit, Layout, Rect};
use crate::art;
use crate::palette::Rgb;

/// Colours as 0–1 floats.
pub type Rgbf = [f32; 3];

fn hex(c: u32) -> Rgbf {
    [(c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF].map(|v| v as f32 / 255.0)
}

/// Light or dark, as Windows' apps are.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    pub bg: Rgbf,
    pub card: Rgbf,
    pub field: Rgbf,
    pub border: Rgbf,
    pub text: Rgbf,
    pub subtext: Rgbf,
    pub button: Rgbf,
    pub button_hover: Rgbf,
}

impl Theme {
    pub fn new(dark: bool) -> Theme {
        if dark {
            Theme {
                dark,
                bg: hex(0x202020),
                card: hex(0x2B2B2B),
                field: hex(0x1C1C1C),
                border: hex(0x3A3A3A),
                text: hex(0xF2F2F2),
                subtext: hex(0xA6A6A6),
                button: hex(0x343434),
                button_hover: hex(0x3E3E3E),
            }
        } else {
            Theme {
                dark,
                bg: hex(0xF3F3F3),
                card: hex(0xFFFFFF),
                field: hex(0xFFFFFF),
                border: hex(0xE0E0E0),
                text: hex(0x1B1B1B),
                subtext: hex(0x6B6B6B),
                button: hex(0xFBFBFB),
                button_hover: hex(0xF0F0F0),
            }
        }
    }
}

/// Opaque 32-bit pixels, 0x00RRGGBB: a top-down DIB as GDI takes it.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

fn to_u32(c: Rgbf) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(c[0]) << 16) | (b(c[1]) << 8) | b(c[2])
}

fn mix(a: Rgbf, b: Rgbf, t: f32) -> Rgbf {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Signed distance to a rounded box, pixels; negative inside.
fn round_box(x: f32, y: f32, r: &Rect, radius: f32) -> f32 {
    let radius = radius.min(r.w / 2.0).min(r.h / 2.0);
    let qx = (x - r.cx()).abs() - (r.w / 2.0 - radius);
    let qy = (y - r.cy()).abs() - (r.h / 2.0 - radius);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius
}

fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Relative luminance: whether text on `c` should be dark.
pub fn is_light(c: Rgbf) -> bool {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2] > 0.6
}

impl Canvas {
    pub fn new(w: usize, h: usize, bg: Rgbf) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![to_u32(bg); w * h],
        }
    }

    fn blend(&mut self, x: usize, y: usize, c: Rgbf, a: f32) {
        if a <= 0.0 || x >= self.w || y >= self.h {
            return;
        }
        let i = y * self.w + x;
        let old = self.px[i];
        let o = hex(old);
        self.px[i] = if a >= 1.0 {
            to_u32(c)
        } else {
            to_u32(mix(o, c, a))
        };
    }

    /// Calls `shade(x, y)` → (colour, alpha) over the pixels of `bounds`.
    fn each(&mut self, bounds: Rect, mut shade: impl FnMut(f32, f32) -> Option<(Rgbf, f32)>) {
        let x0 = bounds.x.floor().max(0.0) as usize;
        let y0 = bounds.y.floor().max(0.0) as usize;
        let x1 = (bounds.right().ceil() as usize).min(self.w);
        let y1 = (bounds.bottom().ceil() as usize).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                if let Some((c, a)) = shade(x as f32 + 0.5, y as f32 + 0.5) {
                    self.blend(x, y, c, a);
                }
            }
        }
    }

    pub fn round_rect(&mut self, r: Rect, radius: f32, c: Rgbf, alpha: f32) {
        self.each(r.inset(-1.0), |x, y| {
            Some((c, alpha * coverage(round_box(x, y, &r, radius))))
        });
    }

    /// A rounded rectangle with a one-pixel border.
    pub fn framed(&mut self, r: Rect, radius: f32, fill: Rgbf, border: Rgbf) {
        self.round_rect(r, radius, border, 1.0);
        self.round_rect(r.inset(1.0), (radius - 1.0).max(0.0), fill, 1.0);
    }

    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, c: Rgbf, alpha: f32) {
        let b = Rect {
            x: cx - radius - 1.0,
            y: cy - radius - 1.0,
            w: 2.0 * radius + 2.0,
            h: 2.0 * radius + 2.0,
        };
        self.each(b, |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius;
            Some((c, alpha * coverage(d)))
        });
    }

    pub fn ring(&mut self, cx: f32, cy: f32, radius: f32, width: f32, c: Rgbf, alpha: f32) {
        let b = Rect {
            x: cx - radius - width,
            y: cy - radius - width,
            w: 2.0 * (radius + width),
            h: 2.0 * (radius + width),
        };
        self.each(b, |x, y| {
            let d = (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius).abs() - width / 2.0;
            Some((c, alpha * coverage(d)))
        });
    }

    /// Straight-alpha RGBA, `size` square, at (x, y).
    pub fn image(&mut self, rgba: &[u8], size: usize, x: usize, y: usize) {
        for (i, p) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let a = p[3] as f32 / 255.0;
            let c = [p[0], p[1], p[2]].map(|v| v as f32 / 255.0);
            self.blend(x + i % size, y + i / size, c, a);
        }
    }
}

/// What the picker shows.
pub struct State<'a> {
    pub hsv: Hsv,
    pub palette: &'a [Rgb],
    pub recent: &'a [Rgb],
    pub hover: Hit,
}

fn unit(rgb: Rgb) -> Rgbf {
    rgb.0.map(|c| c as f32 / 255.0)
}

pub fn paint(l: &Layout, t: &Theme, s: &State) -> Canvas {
    let k = l.scale;
    let mut c = Canvas::new(l.width as usize, l.height as usize, t.bg);
    let current = s.hsv.to_rgb();
    let colour = unit(current);

    // The folder as it will look, on a card.
    c.framed(l.preview, 12.0 * k, t.card, t.border);
    let size = (l.preview.w * 0.78).round() as usize;
    let folder = art::folder(size as u32, |_| colour);
    let ox = (l.preview.cx() - size as f32 / 2.0).round() as usize;
    let oy = (l.preview.cy() - size as f32 / 2.0).round() as usize;
    c.image(&folder, size, ox, oy);

    // The HEX field's frame; the edit control sits inside it.
    c.framed(l.hex, 8.0 * k, t.field, t.border);

    // Saturation across, value down, at the current hue.
    let sv = l.sv;
    let radius = 10.0 * k;
    let h = s.hsv.h;
    c.each(sv.inset(-1.0), |x, y| {
        let a = coverage(round_box(x, y, &sv, radius));
        if a <= 0.0 {
            return None;
        }
        let sat = ((x - sv.x) / sv.w).clamp(0.0, 1.0);
        let val = 1.0 - ((y - sv.y) / sv.h).clamp(0.0, 1.0);
        Some((rgb_f(h, sat, val), a))
    });
    let hx = sv.x + s.hsv.s * sv.w;
    let hy = sv.y + (1.0 - s.hsv.v) * sv.h;
    handle(&mut c, hx, hy, 9.0 * k, colour, k);

    // The hue bar.
    let bar = l.hue;
    c.each(bar.inset(-1.0), |x, y| {
        let a = coverage(round_box(x, y, &bar, bar.h / 2.0));
        (a > 0.0).then(|| (rgb_f(((x - bar.x) / bar.w).clamp(0.0, 1.0), 1.0, 1.0), a))
    });
    let bx = bar.x + s.hsv.h.clamp(0.0, 1.0) * bar.w;
    handle(
        &mut c,
        bx,
        bar.cy(),
        bar.h / 2.0 + 3.0 * k,
        rgb_f(s.hsv.h, 1.0, 1.0),
        k,
    );

    // Swatches: the palette, then the recent colours.
    let rows = [(s.palette, &l.palette, 0usize), (s.recent, &l.recent, 1)];
    for (colours, rects, row) in rows {
        for (i, (rgb, r)) in colours.iter().zip(rects.iter()).enumerate() {
            let hovered = match (row, s.hover) {
                (0, Hit::Palette(j)) | (1, Hit::Recent(j)) => j == i,
                _ => false,
            };
            let radius = r.w / 2.0;
            c.disc(r.cx(), r.cy(), radius, unit(*rgb), 1.0);
            // A hairline keeps light colours visible on a light window.
            c.ring(r.cx(), r.cy(), radius - 0.5 * k, 1.0 * k, t.text, 0.10);
            if *rgb == current {
                c.ring(r.cx(), r.cy(), radius + 4.0 * k, 2.0 * k, t.text, 0.9);
            } else if hovered {
                c.ring(r.cx(), r.cy(), radius + 4.0 * k, 2.0 * k, t.text, 0.3);
            }
        }
    }
    if s.recent.is_empty() {
        // Nothing yet: a dashed-looking empty slot, the hint is text.
        let r = Rect {
            x: l.recent_row.x,
            y: l.recent_row.y,
            w: l.recent_row.h,
            h: l.recent_row.h,
        };
        c.ring(r.cx(), r.cy(), r.w / 2.0 - k, 1.5 * k, t.subtext, 0.5);
    }

    // Buttons: Cancel quiet, Apply in the colour being chosen.
    let cancel_fill = if s.hover == Hit::Cancel {
        t.button_hover
    } else {
        t.button
    };
    c.framed(l.cancel, 8.0 * k, cancel_fill, t.border);
    let apply_fill = if s.hover == Hit::Apply {
        mix(colour, [0.0; 3], 0.12)
    } else {
        colour
    };
    c.round_rect(l.apply, 8.0 * k, apply_fill, 1.0);
    c
}

/// A draggable knob: white ring with a soft outline, the colour inside.
fn handle(c: &mut Canvas, x: f32, y: f32, radius: f32, fill: Rgbf, k: f32) {
    c.disc(x, y + 1.0 * k, radius + 1.5 * k, [0.0; 3], 0.18);
    c.disc(x, y, radius, [1.0; 3], 1.0);
    c.disc(x, y, radius - 3.0 * k, fill, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Tint;

    fn at(c: &Canvas, x: f32, y: f32) -> Rgbf {
        hex(c.px[y as usize * c.w + x as usize])
    }

    fn state<'a>(hsv: Hsv, palette: &'a [Rgb], recent: &'a [Rgb]) -> State<'a> {
        State {
            hsv,
            palette,
            recent,
            hover: Hit::Nothing,
        }
    }

    #[test]
    fn square_runs_white_to_hue_and_down_to_black() {
        let l = Layout::new(1.0, 0, 0);
        let c = paint(
            &l,
            &Theme::new(false),
            &state(
                Hsv {
                    h: 0.0,
                    s: 0.5,
                    v: 0.5,
                },
                &[],
                &[],
            ),
        );
        let near = |x: f32, y: f32| at(&c, x, y);
        let top_left = near(l.sv.x + 12.0, l.sv.y + 12.0);
        assert!(
            top_left.iter().all(|&v| v > 0.85),
            "near white: {top_left:?}"
        );
        let top_right = near(l.sv.right() - 12.0, l.sv.y + 12.0);
        assert!(
            top_right[0] > 0.9 && top_right[1] < 0.2,
            "near red: {top_right:?}"
        );
        let bottom = near(l.sv.cx() + 40.0, l.sv.bottom() - 3.0);
        assert!(bottom.iter().all(|&v| v < 0.1), "near black: {bottom:?}");
    }

    #[test]
    fn swatches_and_apply_button_show_their_colours() {
        let palette: Vec<Rgb> = Tint::ALL.iter().map(|t| Rgb(t.rgb())).collect();
        let recent = [Rgb([0x3A, 0xC8, 0x78])];
        let hsv = Hsv::from_rgb(recent[0], 0.0);
        let l = Layout::new(1.25, palette.len(), recent.len());
        let c = paint(&l, &Theme::new(true), &state(hsv, &palette, &recent));
        let red = at(&c, l.palette[0].cx(), l.palette[0].cy());
        assert!(red[0] > 0.95 && red[1] < 0.3, "{red:?}");
        let mine = at(&c, l.recent[0].cx(), l.recent[0].cy());
        assert!((mine[1] - 200.0 / 255.0).abs() < 0.01, "{mine:?}");
        let apply = at(&c, l.apply.cx(), l.apply.y + 4.0);
        assert!(
            (apply[1] - 200.0 / 255.0).abs() < 0.01,
            "apply is the chosen colour: {apply:?}"
        );
        // Dark theme background.
        assert!(at(&c, 2.0, 2.0)[0] < 0.2);
    }

    #[test]
    fn text_contrast_follows_luminance() {
        assert!(is_light(unit(Rgb(Tint::Yellow.rgb()))));
        assert!(!is_light(unit(Rgb(Tint::Blue.rgb()))));
    }
}
