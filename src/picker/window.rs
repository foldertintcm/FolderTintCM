//! The picker's window: a plain Win32 window whose client area is painted
//! by `paint.rs` into a DIB, with GDI text on top and one real EDIT
//! control for the HEX code. Modal in the sense that matters here: the
//! process has nothing else to do, so `run` pumps messages until the user
//! applies or cancels.

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, GetMonitorInfoW, HBRUSH, HDC, HFONT,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, PCWSTR, w};

use super::hsv::Hsv;
use super::layout::{Hit, Layout};
use super::paint::{self, Canvas, Rgbf, Theme};
use crate::i18n::tr;
use crate::palette::Rgb;
use crate::wide::{from_buffer, wide};

const EDIT_ID: usize = 100;
const EN_CHANGE: u32 = 0x0300;
const EM_LIMITTEXT: u32 = 0x00C5;
const ES_UPPERCASE: u32 = 0x0008;
const ES_AUTOHSCROLL: u32 = 0x0080;
/// winuser.h; the crate files it under Controls, a feature this needs
/// for nothing else.
const WM_MOUSELEAVE: u32 = 0x02A3;

struct Picker {
    hsv: Hsv,
    palette: Vec<Rgb>,
    recent: Vec<Rgb>,
    layout: Layout,
    theme: Theme,
    hover: Hit,
    drag: Option<Hit>,
    tracking: bool,
    /// `Some` once the user has answered.
    answer: Option<Option<Rgb>>,
    edit: HWND,
    /// Set while the program writes the HEX field, so its change
    /// notification is not taken for typing.
    writing_edit: bool,
    fonts: Fonts,
    field_brush: HBRUSH,
}

struct Fonts {
    body: HFONT,
    strong: HFONT,
    small: HFONT,
}

impl Fonts {
    fn new(scale: f32) -> Fonts {
        let make = |dip: f32, weight: i32| unsafe {
            CreateFontW(
                -(dip * scale).round() as i32,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI"),
            )
        };
        Fonts {
            body: make(14.0, 400),
            strong: make(14.0, 600),
            small: make(12.0, 400),
        }
    }

    fn delete(&self) {
        unsafe {
            for f in [self.body, self.strong, self.small] {
                let _ = DeleteObject(f.into());
            }
        }
    }
}

fn colorref(c: Rgbf) -> COLORREF {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    COLORREF(b(c[0]) | (b(c[1]) << 8) | (b(c[2]) << 16))
}

thread_local! {
    static PICKER: RefCell<Option<Picker>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Picker) -> R) -> Option<R> {
    PICKER.with(|p| p.try_borrow_mut().ok().and_then(|mut p| p.as_mut().map(f)))
}

/// Opens the picker on `initial` and waits for the answer: `Some(colour)`
/// on Apply (or Enter), `None` on Cancel, Escape or closing the window.
pub fn run(initial: Rgb, palette: Vec<Rgb>, recent: Vec<Rgb>, dark: bool) -> Option<Rgb> {
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hIcon: LoadIconW(Some(instance.into()), PCWSTR(1 as _)).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("FolderTint.Picker"),
            ..Default::default()
        };
        RegisterClassExW(&class);

        let theme = Theme::new(dark);
        PICKER.with(|p| {
            *p.borrow_mut() = Some(Picker {
                hsv: Hsv::from_rgb(initial, 0.0),
                layout: Layout::new(1.0, palette.len(), recent.len()),
                palette,
                recent,
                theme,
                hover: Hit::Nothing,
                drag: None,
                tracking: false,
                answer: None,
                edit: HWND::default(),
                writing_edit: false,
                fonts: Fonts::new(1.0),
                field_brush: CreateSolidBrush(colorref(theme.field)),
            })
        });

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let title = wide(tr(
            "Color personalizado — FolderTint",
            "Custom colour — FolderTint",
        ))?;
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("FolderTint.Picker"),
            PCWSTR(title.as_ptr()),
            style,
            cursor.x,
            cursor.y,
            100,
            100,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;

        frame(hwnd, &theme);
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_UPPERCASE | ES_AUTOHSCROLL),
            0,
            0,
            10,
            10,
            Some(hwnd),
            Some(HMENU(EDIT_ID as _)),
            Some(instance.into()),
            None,
        )
        .ok()?;
        SendMessageW(edit, EM_LIMITTEXT, Some(WPARAM(7)), None);
        with(|p| p.edit = edit);

        // Sized for the monitor the window opened on, centred on it.
        relayout(hwnd, GetDpiForWindow(hwnd));
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut outer = RECT::default();
        let _ = GetWindowRect(hwnd, &mut outer);
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let work = info.rcWork;
            let (w, h) = (outer.right - outer.left, outer.bottom - outer.top);
            let x = work.left + ((work.right - work.left) - w) / 2;
            let y = work.top + ((work.bottom - work.top) - h) / 2;
            let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
        }
        write_edit();
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while with(|p| p.answer.is_none()).unwrap_or(false)
            && GetMessageW(&mut msg, None, 0, 0).as_bool()
        {
            // Enter and Escape act on the window wherever the focus is,
            // the HEX field included.
            if msg.message == WM_KEYDOWN {
                match msg.wParam.0 as u16 {
                    k if k == VK_RETURN.0 => {
                        finish(true);
                        continue;
                    }
                    k if k == VK_ESCAPE.0 => {
                        finish(false);
                        continue;
                    }
                    _ => {}
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DestroyWindow(hwnd);
        let picker = PICKER.with(|p| p.borrow_mut().take())?;
        picker.fonts.delete();
        let _ = DeleteObject(picker.field_brush.into());
        picker.answer.flatten()
    }
}

/// Title bar in the window's own background, dark or light.
fn frame(hwnd: HWND, theme: &Theme) {
    let dark = BOOL(theme.dark as i32);
    let caption = colorref(theme.bg);
    let text = colorref(theme.text);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as _,
            4,
        );
        // Windows 11; ignored by Windows 10.
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption as *const _ as _, 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &text as *const _ as _, 4);
    }
}

/// Lays the window out for `dpi`: client size, fonts, the HEX field.
fn relayout(hwnd: HWND, dpi: u32) {
    // A debug build takes `FT_SCALE=2` to photograph the picker sharp on a
    // 100 % monitor.
    let forced = std::env::var("FT_SCALE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok());
    let scale = match forced {
        Some(s) if cfg!(debug_assertions) && (1.0..=3.0).contains(&s) => s,
        _ => dpi.max(96) as f32 / 96.0,
    };
    let Some((w, h, edit, font, hex)) = with(|p| {
        p.layout = Layout::new(scale, p.palette.len(), p.recent.len());
        p.fonts.delete();
        p.fonts = Fonts::new(scale);
        (
            p.layout.width,
            p.layout.height,
            p.edit,
            p.fonts.body,
            p.layout.hex,
        )
    }) else {
        return;
    };
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        let mut r = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE::default(), dpi);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            r.right - r.left,
            r.bottom - r.top,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        // The edit control: right of the "HEX" caption, centred in height.
        let line = (20.0 * scale).round() as i32;
        let x = (hex.x + 52.0 * scale).round() as i32;
        let y = (hex.cy() - line as f32 / 2.0).round() as i32;
        let width = (hex.right() - 12.0 * scale).round() as i32 - x;
        let _ = SetWindowPos(edit, None, x, y, width, line, SWP_NOZORDER);
        SendMessageW(
            edit,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

/// Puts the current colour's code in the HEX field.
fn write_edit() {
    let Some((edit, text)) = with(|p| {
        p.writing_edit = true;
        (p.edit, p.hsv.to_rgb().label())
    }) else {
        return;
    };
    if let Some(t) = wide(&text) {
        unsafe {
            let _ = SetWindowTextW(edit, PCWSTR(t.as_ptr()));
        }
    }
    with(|p| p.writing_edit = false);
}

/// `#rrggbb` or `rrggbb`, as typed.
fn parse_typed(text: &str) -> Option<Rgb> {
    let t = text.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    Rgb::parse(&format!("#{t}"))
}

fn finish(apply: bool) {
    with(|p| p.answer = Some(apply.then(|| p.hsv.to_rgb())));
}

fn set_colour(rgb: Rgb) {
    with(|p| p.hsv = Hsv::from_rgb(rgb, p.hsv.h));
    write_edit();
}

/// Acts on a press or a drag at (x, y).
fn press(hwnd: HWND, hit: Hit, x: f32, y: f32) {
    match hit {
        Hit::Sv => {
            with(|p| {
                let (s, v) = p.layout.sv_at(x, y);
                p.hsv.s = s;
                p.hsv.v = v;
            });
            write_edit();
        }
        Hit::Hue => {
            with(|p| p.hsv.h = p.layout.hue_at(x));
            write_edit();
        }
        Hit::Palette(i) => {
            if let Some(rgb) = with(|p| p.palette.get(i).copied()).flatten() {
                set_colour(rgb);
            }
        }
        Hit::Recent(i) => {
            if let Some(rgb) = with(|p| p.recent.get(i).copied()).flatten() {
                set_colour(rgb);
            }
        }
        Hit::Cancel => finish(false),
        Hit::Apply => finish(true),
        Hit::Nothing => {}
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn point(l: LPARAM) -> (f32, f32) {
    let x = (l.0 & 0xFFFF) as u16 as i16 as f32;
    let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as f32;
    (x, y)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint_window(hwnd);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_LBUTTONDOWN => {
                let (x, y) = point(lparam);
                let hit = with(|p| p.layout.hit(x, y)).unwrap_or(Hit::Nothing);
                if matches!(hit, Hit::Sv | Hit::Hue) {
                    SetCapture(hwnd);
                    with(|p| p.drag = Some(hit));
                }
                // Clicking the colours takes the focus from the HEX field.
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
                press(hwnd, hit, x, y);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = point(lparam);
                let (drag, changed, track) = with(|p| {
                    let hover = p.layout.hit(x, y);
                    let changed = hover != p.hover;
                    p.hover = hover;
                    let track = !p.tracking;
                    p.tracking = true;
                    (p.drag, changed, track)
                })
                .unwrap_or((None, false, false));
                if track {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }
                if let Some(target) = drag {
                    press(hwnd, target, x, y);
                } else if changed {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                with(|p| p.drag = None);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with(|p| {
                    p.hover = Hit::Nothing;
                    p.tracking = false;
                });
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_SETCURSOR
                if HWND(wparam.0 as _) == hwnd && (lparam.0 & 0xFFFF) as u32 == HTCLIENT =>
            {
                let hover = with(|p| p.hover).unwrap_or(Hit::Nothing);
                let cursor = match hover {
                    Hit::Palette(_) | Hit::Recent(_) | Hit::Cancel | Hit::Apply => IDC_HAND,
                    Hit::Sv | Hit::Hue => IDC_CROSS,
                    Hit::Nothing => IDC_ARROW,
                };
                SetCursor(LoadCursorW(None, cursor).ok());
                LRESULT(1)
            }
            WM_COMMAND
                if (wparam.0 & 0xFFFF) == EDIT_ID && ((wparam.0 >> 16) as u32) == EN_CHANGE =>
            {
                let (edit, writing) = with(|p| (p.edit, p.writing_edit)).unwrap_or_default();
                if !writing {
                    let mut buf = [0u16; 16];
                    let n = GetWindowTextW(edit, &mut buf);
                    if let Some(rgb) = parse_typed(&from_buffer(&buf[..n.max(0) as usize])) {
                        // Typing must not rewrite what is being typed.
                        with(|p| p.hsv = Hsv::from_rgb(rgb, p.hsv.h));
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_CTLCOLOREDIT => {
                let hdc = HDC(wparam.0 as _);
                let (theme, brush) = with(|p| (p.theme, p.field_brush)).unzip();
                if let (Some(t), Some(b)) = (theme, brush) {
                    SetTextColor(hdc, colorref(t.text));
                    SetBkColor(hdc, colorref(t.field));
                    return LRESULT(b.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DPICHANGED => {
                relayout(hwnd, (wparam.0 & 0xFFFF) as u32);
                let suggested = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER,
                );
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_CLOSE => {
                finish(false);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn paint_window(hwnd: HWND) {
    let Some((canvas, texts)) = with(|p| {
        let state = paint::State {
            hsv: p.hsv,
            palette: &p.palette,
            recent: &p.recent,
            hover: p.hover,
        };
        (paint::paint(&p.layout, &p.theme, &state), texts(p))
    }) else {
        return;
    };
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        blit(hdc, &canvas, &texts);
        let _ = EndPaint(hwnd, &ps);
    }
}

struct Text {
    text: String,
    rect: RECT,
    font: HFONT,
    colour: Rgbf,
    centred: bool,
}

fn rect(r: super::layout::Rect) -> RECT {
    RECT {
        left: r.x.round() as i32,
        top: r.y.round() as i32,
        right: r.right().round() as i32,
        bottom: r.bottom().round() as i32,
    }
}

fn texts(p: &Picker) -> Vec<Text> {
    let l = &p.layout;
    let t = &p.theme;
    let k = l.scale;
    let rgb = p.hsv.to_rgb();
    let [r, g, b] = rgb.0;
    let on_apply = if paint::is_light(rgb.0.map(|c| c as f32 / 255.0)) {
        [0.1; 3]
    } else {
        [1.0; 3]
    };
    let mut hex_label = l.hex;
    hex_label.x += 12.0 * k;
    let mut out = vec![
        Text {
            text: "HEX".into(),
            rect: rect(hex_label),
            font: p.fonts.small,
            colour: t.subtext,
            centred: false,
        },
        Text {
            text: format!("R {r}   G {g}   B {b}"),
            rect: rect(l.rgb_text),
            font: p.fonts.small,
            colour: t.subtext,
            centred: false,
        },
        Text {
            text: tr("Colores", "Colours").into(),
            rect: rect(l.palette_label),
            font: p.fonts.strong,
            colour: t.text,
            centred: false,
        },
        Text {
            text: tr("Recientes", "Recent").into(),
            rect: rect(l.recent_label),
            font: p.fonts.strong,
            colour: t.text,
            centred: false,
        },
        Text {
            text: tr("Cancelar", "Cancel").into(),
            rect: rect(l.cancel),
            font: p.fonts.body,
            colour: t.text,
            centred: true,
        },
        Text {
            text: tr("Aplicar", "Apply").into(),
            rect: rect(l.apply),
            font: p.fonts.strong,
            colour: on_apply,
            centred: true,
        },
    ];
    if p.recent.is_empty() {
        let mut hint = l.recent_row;
        hint.x += hint.h + 12.0 * k;
        out.push(Text {
            text: tr(
                "Los colores que cree aparecerán aquí",
                "Colours you create will appear here",
            )
            .into(),
            rect: rect(hint),
            font: p.fonts.body,
            colour: t.subtext,
            centred: false,
        });
    }
    out
}

/// The painted pixels and the text, through a memory DC, to the window.
unsafe fn blit(hdc: HDC, canvas: &Canvas, texts: &[Text]) {
    unsafe {
        let (w, h) = (canvas.w as i32, canvas.h as i32);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mem = CreateCompatibleDC(Some(hdc));
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(mem);
            return;
        };
        std::ptr::copy_nonoverlapping(canvas.px.as_ptr(), bits as *mut u32, canvas.px.len());
        let old = SelectObject(mem, bitmap.into());
        SetBkMode(mem, TRANSPARENT);
        for t in texts {
            let Some(mut text) = wide(&t.text) else {
                continue;
            };
            text.pop(); // DrawTextW takes the length, not the NUL
            let mut r = t.rect;
            let align = if t.centred { DT_CENTER } else { DT_LEFT };
            SelectObject(mem, t.font.into());
            SetTextColor(mem, colorref(t.colour));
            DrawTextW(
                mem,
                &mut text,
                &mut r,
                align | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
        let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_codes_with_or_without_hash() {
        assert_eq!(parse_typed("#3ac878"), Some(Rgb([0x3A, 0xC8, 0x78])));
        assert_eq!(parse_typed(" 3AC878 "), Some(Rgb([0x3A, 0xC8, 0x78])));
        assert_eq!(parse_typed("#3AC87"), None);
        assert_eq!(parse_typed("##3AC878"), None);
    }
}
