//! Spanish and English, as in ScreenShotCM: every string the user sees is
//! written as `tr("español", "English")` where it is used. The context menu
//! itself is not here — its strings are resources in the executable (see
//! `build.rs`), which Windows picks by display language on its own.

use windows::Win32::Globalization::GetUserDefaultUILanguage;

/// Spanish when Windows is in Spanish, English otherwise. A debug build
/// takes `FT_LANG=es|en`, to photograph the picker in both languages.
fn spanish() -> bool {
    if cfg!(debug_assertions)
        && let Some(lang) = std::env::var_os("FT_LANG")
    {
        return lang == "es";
    }
    // The primary language is the low ten bits of the LANGID; 0x0A is
    // Spanish in every country.
    (unsafe { GetUserDefaultUILanguage() } & 0x3FF) == 0x0A
}

pub fn tr(es: &'static str, en: &'static str) -> &'static str {
    if spanish() { es } else { en }
}
