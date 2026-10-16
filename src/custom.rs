//! Colours beyond the palette: picking or creating one in FolderTint's own
//! picker, drawing its icon on first use, and keeping the last few in the
//! context menu so they are one click away next time.
//!
//! Kept under `HKCU\Software\FolderTint\Custom`:
//! - `Recent`: the last colours used, newest first (`3a7bd5,ff8800`).
//! - (`Swatches`, the 16 slots of the Windows dialog 0.2.0 used, is no
//!   longer written; `forget_all` still removes it with the key.)
//! - `LastPick`: the latest answer of the dialog, for the other folders of a
//!   multiple selection (see `pick_shared`).

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject,
};
use windows::core::w;

use crate::installer::PLACES;
use crate::palette::Rgb;
use crate::{art, ico, reg};

const KEY: &str = r"Software\FolderTint\Custom";

/// How many recent colours the menu offers.
pub const MAX_RECENT: usize = 6;

/// Shown with a line above it.
const ECF_SEPARATORBEFORE: u32 = 0x20;

/// The `.ico` of a custom colour, drawn the first time it is used.
pub fn ensure_icon(data_dir: &Path, rgb: Rgb) -> io::Result<PathBuf> {
    let path = crate::paths::custom_icon(data_dir, rgb);
    if path.is_file() {
        return Ok(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Several folders coloured at once run several processes: each writes
    // its own file and moves it into place, so none reads a half-written one.
    let colour = art::unit_rgb(rgb.0);
    let bytes = ico::ico(|size| art::folder(size, |_| colour));
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// The recent colours, newest first. A debug build takes
/// `FT_RECENT=3a7bd5,ff8800` instead, to photograph the picker without
/// touching the user's list.
pub fn recent() -> Vec<Rgb> {
    if cfg!(debug_assertions)
        && let Ok(forced) = std::env::var("FT_RECENT")
    {
        return parse_recent(&forced);
    }
    parse_recent(&reg::get_string(KEY, Some("Recent")).unwrap_or_default())
}

fn parse_recent(s: &str) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::new();
    for rgb in s
        .split(',')
        .filter_map(|h| Rgb::parse(&format!("#{}", h.trim())))
    {
        if !out.contains(&rgb) && out.len() < MAX_RECENT {
            out.push(rgb);
        }
    }
    out
}

/// `rgb` first, then the others in order, without repeats, at most
/// `MAX_RECENT`.
fn promote(list: &[Rgb], rgb: Rgb) -> Vec<Rgb> {
    std::iter::once(rgb)
        .chain(list.iter().copied().filter(|&c| c != rgb))
        .take(MAX_RECENT)
        .collect()
}

/// Puts `rgb` first among the recent colours and rewrites their menu
/// entries. `exe` is the program the entries run.
pub fn remember(rgb: Rgb, exe: &Path, data_dir: &Path) {
    let list = promote(&recent(), rgb);
    let joined: Vec<String> = list.iter().map(|c| c.hex()).collect();
    let _ = reg::set_string(KEY, Some("Recent"), &joined.join(","));
    sync_menu(&list, exe, data_dir);
}

/// The menu key of the `i`-th recent colour under a place. Explorer sorts
/// sub-verbs by key name: "50" to "55" fall between "Personalizado…"
/// ("14custom") and "Quitar color" ("99clear").
fn recent_key(place_key: &str, i: usize) -> String {
    format!(r"{place_key}\shell\5{i}")
}

fn sync_menu(list: &[Rgb], exe: &Path, data_dir: &Path) {
    let exe = exe.display();
    for place in &PLACES {
        // Only where the installer put the menu: a copy of the program run
        // from elsewhere (a build folder) leaves the menu alone.
        if !reg::exists(place.key) {
            continue;
        }
        for i in 0..MAX_RECENT {
            let key = recent_key(place.key, i);
            let Some(&rgb) = list.get(i) else {
                reg::delete_tree(&key);
                continue;
            };
            let icon = crate::paths::custom_icon(data_dir, rgb);
            let command = format!("\"{exe}\" --apply #{} \"{}\"", rgb.hex(), place.folder_arg);
            let _ = reg::set_string(&key, Some("MUIVerb"), &rgb.label());
            let _ = reg::set_string(&key, Some("Icon"), &icon.display().to_string());
            if i == 0 {
                let _ = reg::set_dword(&key, "CommandFlags", ECF_SEPARATORBEFORE);
            }
            let _ = reg::set_string(&format!(r"{key}\command"), None, &command);
        }
    }
}

/// Uninstalling: the recent colours' menu entries and everything kept here.
pub fn forget_all() {
    for place in &PLACES {
        for i in 0..MAX_RECENT {
            reg::delete_tree(&recent_key(place.key, i));
        }
    }
    reg::delete_tree(KEY);
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// The colour dialog, once for a whole selection. Explorer runs the verb
/// once per selected folder, all at the same moment: the first process to
/// get the lock shows the dialog and writes the answer down; the others,
/// started before that answer, wait for the lock and take it, colour or
/// cancel, instead of opening a dialog each.
pub fn pick_shared(initial: Rgb) -> Option<Rgb> {
    let started = now_ms();
    let mutex = unsafe { CreateMutexW(None, false, w!("Local\\FolderTint.Pick")) }.ok();
    if let Some(m) = mutex {
        unsafe {
            WaitForSingleObject(m, INFINITE);
        }
    }
    let answer = match last_pick() {
        Some((at, answer)) if at >= started => answer,
        _ => {
            let answer = crate::picker::pick(initial, recent());
            let value = format!(
                "{}|{}",
                now_ms(),
                answer.map_or("cancel".to_owned(), Rgb::hex)
            );
            let _ = reg::set_string(KEY, Some("LastPick"), &value);
            answer
        }
    };
    if let Some(m) = mutex {
        unsafe {
            let _ = ReleaseMutex(m);
            let _ = CloseHandle(m);
        }
    }
    answer
}

/// (when, colour or cancel) of the latest dialog.
fn last_pick() -> Option<(u128, Option<Rgb>)> {
    parse_last_pick(&reg::get_string(KEY, Some("LastPick"))?)
}

fn parse_last_pick(s: &str) -> Option<(u128, Option<Rgb>)> {
    let (at, what) = s.split_once('|')?;
    let at = at.parse().ok()?;
    match what {
        "cancel" => Some((at, None)),
        hex => Rgb::parse(&format!("#{hex}")).map(|c| (at, Some(c))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_list_is_deduplicated_and_capped() {
        let parsed = parse_recent("aa0000, bb0000,aa0000,zz,cc0000,dd0000,ee0000,ff0000,110000");
        assert_eq!(parsed.len(), MAX_RECENT);
        assert_eq!(parsed[0], Rgb([0xAA, 0, 0]));
        assert_eq!(parsed[1], Rgb([0xBB, 0, 0]));
        assert!(parse_recent("").is_empty());
    }

    #[test]
    fn promoting_moves_to_front_without_repeats() {
        let list = parse_recent("aa0000,bb0000,cc0000");
        let up = promote(&list, Rgb([0xCC, 0, 0]));
        assert_eq!(up, parse_recent("cc0000,aa0000,bb0000"));
        let full = parse_recent("010101,020202,030303,040404,050505,060606");
        let new = promote(&full, Rgb([9, 9, 9]));
        assert_eq!(new.len(), MAX_RECENT);
        assert_eq!(new[0], Rgb([9, 9, 9]));
        assert!(!new.contains(&Rgb([6, 6, 6])), "oldest dropped");
    }

    #[test]
    fn last_pick_reads_colour_or_cancel() {
        assert_eq!(
            parse_last_pick("123|3a7bd5"),
            Some((123, Some(Rgb([0x3A, 0x7B, 0xD5]))))
        );
        assert_eq!(parse_last_pick("5|cancel"), Some((5, None)));
        assert_eq!(parse_last_pick("x|cancel"), None);
        assert_eq!(parse_last_pick("5|zzzzzz"), None);
        assert_eq!(parse_last_pick(""), None);
    }

    #[test]
    fn recent_keys_sort_between_custom_and_clear() {
        let k = recent_key("X", 0);
        let name = k.rsplit('\\').next().unwrap();
        assert!("14custom" < name && name < "99clear");
        assert!(recent_key("X", MAX_RECENT - 1).rsplit('\\').next().unwrap() < "99clear");
    }

    #[test]
    fn icons_are_drawn_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("foldertint-custom-{}", std::process::id()));
        let rgb = Rgb([0x12, 0x34, 0x56]);
        let path = ensure_icon(&dir, rgb).unwrap();
        assert!(path.ends_with(r"custom\123456.ico"));
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(ensure_icon(&dir, rgb).unwrap(), path);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
        assert_eq!(&std::fs::read(&path).unwrap()[..4], &[0, 0, 1, 0]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
