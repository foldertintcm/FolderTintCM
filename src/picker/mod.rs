//! FolderTintCM's own colour picker, in place of Windows' 1990s ChooseColor
//! dialog (which also came out garbled at 150 % scaling): a live preview of
//! the folder, a saturation × value square, a hue bar, the palette and the
//! recent colours as swatches, a HEX field, and Apply in the colour itself.
//! Light or dark as Windows' apps are.

mod hsv;
mod layout;
mod paint;
mod window;

use crate::palette::{Rgb, Tint};
use crate::reg;

/// Shows the picker on `initial`; the chosen colour, or `None` if cancelled.
pub fn pick(initial: Rgb, recent: Vec<Rgb>) -> Option<Rgb> {
    let palette = Tint::ALL.iter().map(|t| Rgb(t.rgb())).collect();
    window::run(initial, palette, recent, apps_use_dark())
}

/// Windows' own "app mode" setting; light when it cannot be read. A debug
/// build takes `FT_THEME=light|dark`, so both can be looked at without
/// touching the user's Windows setting (as ScreenShotCM's `SCM_THEME`).
fn apps_use_dark() -> bool {
    if cfg!(debug_assertions)
        && let Some(forced) = std::env::var_os("FT_THEME")
    {
        return forced == "dark";
    }
    reg::get_dword(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    ) == Some(0)
}
