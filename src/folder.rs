//! Giving a folder an icon, and taking it back, through the shell's own
//! folder customisation (`SHGetSetFolderCustomSettings`) — the same call
//! as Properties → Customize → Change Icon. Windows writes `desktop.ini`,
//! hides it and marks the folder so Explorer reads it; we never write the
//! file ourselves, so whatever else it holds (a localised name, a view
//! state) is kept.

use std::fmt;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_READONLY, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::UI::Shell::{
    FCS_FORCEWRITE, FCS_READ, FCSM_ICONFILE, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_FLUSH,
    SHCNF_PATHW, SHChangeNotify, SHFOLDERCUSTOMSETTINGS, SHGetSetFolderCustomSettings,
};
use windows::core::{PCWSTR, PWSTR};

use crate::palette::{Colour, Rgb, Tint};
use crate::wide::{from_buffer, wide};

#[derive(Debug)]
pub enum Error {
    NotAFolder(PathBuf),
    /// `D:\`, `\\server\share\`: Explorer draws a drive with its own icon
    /// and never reads a `desktop.ini` there, so a colour would not show.
    DriveRoot(PathBuf),
    Windows(windows::core::Error),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAFolder(p) | Error::DriveRoot(p) => write!(f, "{}", p.display()),
            Error::Windows(e) => write!(f, "{}", e.message()),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Windows(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// A real, existing directory — not a file, not a dangling path. Explorer
/// only offers the menu on folders, but the command line is public.
fn checked(folder: &Path) -> Result<Vec<u16>, Error> {
    let not_a_folder = || Error::NotAFolder(folder.to_path_buf());
    match std::fs::metadata(folder) {
        Ok(m) if m.is_dir() => wide(folder).ok_or_else(not_a_folder),
        _ => Err(not_a_folder()),
    }
}

/// A folder that can take a colour: an existing directory that is not the
/// root of a drive. Checked before the picker opens, so nobody chooses a
/// colour that cannot show.
pub fn colourable(folder: &Path) -> Result<(), Error> {
    if folder.parent().is_none() {
        return Err(Error::DriveRoot(folder.to_path_buf()));
    }
    checked(folder).map(drop)
}

/// Sets the folder's icon to `icon` (a path, environment variables
/// allowed, as `desktop.ini` takes them).
pub fn set_icon(folder: &Path, icon: &str) -> Result<(), Error> {
    colourable(folder)?;
    let path = checked(folder)?;
    let mut icon = wide(icon).ok_or_else(|| Error::NotAFolder(folder.to_path_buf()))?;
    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(icon.as_mut_ptr()),
        iIconIndex: 0,
        ..Default::default()
    };
    unsafe { SHGetSetFolderCustomSettings(&mut fcs, PCWSTR(path.as_ptr()), FCS_FORCEWRITE)? };
    notify(&path);
    Ok(())
}

/// The folder's custom icon, if it has one, environment variables expanded.
pub fn icon(folder: &Path) -> Result<Option<String>, Error> {
    let path = checked(folder)?;
    let mut buf = vec![0u16; 2048];
    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(buf.as_mut_ptr()),
        cchIconFile: buf.len() as u32,
        ..Default::default()
    };
    // A folder never customised has no desktop.ini: that is "no icon",
    // not an error.
    if unsafe { SHGetSetFolderCustomSettings(&mut fcs, PCWSTR(path.as_ptr()), FCS_READ) }.is_err() {
        return Ok(None);
    }
    let icon = from_buffer(&buf);
    Ok((!icon.trim().is_empty()).then_some(icon))
}

/// Whether `icon` is one of FolderTintCM's, spelled with an environment
/// variable or not.
pub fn is_ours(icon: &str) -> bool {
    colour_of(icon).is_some()
}

/// The colour a FolderTintCM icon draws: `…\FolderTintCM\icons\<tint>.ico` for
/// the palette, `…\FolderTintCM\custom\<rrggbb>.ico` for a picked colour;
/// `None` for any other icon.
pub fn colour_of(icon: &str) -> Option<Colour> {
    let lower = icon.to_ascii_lowercase().replace('/', "\\");
    let mut parts = lower.rsplit('\\');
    let (Some(file), Some(dir), Some("foldertintcm")) = (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let stem = file.strip_suffix(".ico")?;
    match dir {
        "icons" => Tint::from_slug(stem).map(Colour::Tint),
        "custom" => Rgb::parse(&format!("#{stem}")).map(Colour::Custom),
        _ => None,
    }
}

/// What `clear` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Cleared {
    Restored,
    /// No icon, or one FolderTintCM did not set: left as it was.
    NotOurs,
}

/// Gives the folder back its normal icon, if FolderTintCM coloured it. Never
/// undoes a customisation someone else made (a system folder's icon, one
/// chosen in Properties).
pub fn clear(folder: &Path) -> Result<Cleared, Error> {
    match icon(folder)? {
        Some(icon) if is_ours(&icon) => {}
        _ => return Ok(Cleared::NotOurs),
    }
    let path = checked(folder)?;
    // Writing an empty icon file removes the entry from desktop.ini.
    let mut empty = [0u16; 1];
    let mut fcs = SHFOLDERCUSTOMSETTINGS {
        dwSize: size_of::<SHFOLDERCUSTOMSETTINGS>() as u32,
        dwMask: FCSM_ICONFILE,
        pszIconFile: PWSTR(empty.as_mut_ptr()),
        ..Default::default()
    };
    unsafe { SHGetSetFolderCustomSettings(&mut fcs, PCWSTR(path.as_ptr()), FCS_FORCEWRITE)? };
    tidy(folder)?;
    notify(&path);
    Ok(Cleared::Restored)
}

/// Removes a `desktop.ini` left with nothing in it but section headers,
/// and the read-only mark Windows put on the folder for it, so a folder
/// coloured and uncoloured is as it was.
fn tidy(folder: &Path) -> Result<(), Error> {
    let ini = folder.join("desktop.ini");
    let Ok(bytes) = std::fs::read(&ini) else {
        return Ok(());
    };
    if !ini_is_empty(&decode_ini(&bytes)) {
        return Ok(());
    }
    if let Some(w) = wide(&ini) {
        // Hidden and system files cannot be deleted as they are.
        unsafe { SetFileAttributesW(PCWSTR(w.as_ptr()), FILE_ATTRIBUTE_NORMAL)? };
    }
    std::fs::remove_file(&ini)?;
    let attrs = std::fs::metadata(folder)?.file_attributes();
    if attrs & FILE_ATTRIBUTE_READONLY.0 != 0
        && let Some(w) = wide(folder)
    {
        let keep = FILE_FLAGS_AND_ATTRIBUTES(attrs & !FILE_ATTRIBUTE_READONLY.0);
        unsafe { SetFileAttributesW(PCWSTR(w.as_ptr()), keep)? };
    }
    Ok(())
}

/// `desktop.ini` is UTF-16 when Windows wrote non-ASCII into it, the ANSI
/// code page otherwise; for "is anything left" either reading will do.
fn decode_ini(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Nothing but blank lines, comments, section headers and empty icon keys.
fn ini_is_empty(text: &str) -> bool {
    text.lines().map(str::trim).all(|line| {
        line.is_empty()
            || line.starts_with(';')
            || (line.starts_with('[') && line.ends_with(']'))
            || line
                .split_once('=')
                .is_some_and(|(k, v)| empty_icon_key(k, v))
    })
}

/// An icon key with no file: `IconResource=,0` is what clearing leaves.
fn empty_icon_key(key: &str, value: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    let file = value
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
    let file = file.strip_suffix(',').unwrap_or(file).trim();
    matches!(key.as_str(), "iconresource" | "iconfile" | "iconindex")
        && (file.is_empty() || key == "iconindex")
}

/// Tells Explorer windows showing the folder, or its parent, to redraw it.
fn notify(path: &[u16]) {
    unsafe {
        SHChangeNotify(
            SHCNE_UPDATEITEM,
            SHCNF_PATHW,
            Some(path.as_ptr().cast()),
            None,
        );
        SHChangeNotify(
            SHCNE_UPDATEDIR,
            SHCNF_PATHW,
            Some(path.as_ptr().cast()),
            None,
        );
    }
}

/// Tells Explorer again, once it has caught up with the change on disk.
///
/// Explorer also watches the disk, and handles writing `desktop.ini` and
/// marking the folder on its own schedule. In the views that draw folders
/// from the thumbnail cache (medium icons and larger), a redraw asked for
/// before that is lost: the folder keeps its old icon until something
/// else refreshes it, so the first click seemed to do nothing. Measured on
/// Windows 11: asked again after 200 ms, most folders still kept the old
/// icon; after 600 ms, every one changed. So the program asks again at
/// 0.7 s, and once more at 2 s for a slower or busier machine. It has no
/// window by then, and the folder already has its colour on disk.
pub fn renotify(folder: &Path) {
    const AFTER: [Duration; 2] = [Duration::from_millis(700), Duration::from_millis(1300)];
    let Some(path) = wide(folder) else {
        return;
    };
    for wait in AFTER {
        std::thread::sleep(wait);
        unsafe {
            SHChangeNotify(
                SHCNE_UPDATEITEM,
                SHCNF_PATHW | SHCNF_FLUSH,
                Some(path.as_ptr().cast()),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_only_our_icons() {
        assert!(is_ours(
            r"%LOCALAPPDATA%\Programs\FolderTintCM\icons\red.ico"
        ));
        assert!(is_ours(
            r"C:\Users\x\AppData\Local\Programs\FolderTintCM\icons\GRAY.ICO"
        ));
        assert!(!is_ours(r"%SystemRoot%\system32\imageres.dll"));
        assert!(!is_ours(r"C:\Other\icons\red.ico"));
        assert!(!is_ours(r"C:\FolderTintCM\icons\chartreuse.ico"));
        assert!(!is_ours("red.ico"));
        assert!(!is_ours(""));
    }

    #[test]
    fn reads_the_colour_back_from_the_icon() {
        assert_eq!(
            colour_of(r"C:\U\AppData\Local\FolderTintCM\custom\3A7BD5.ico"),
            Some(Colour::Custom(Rgb([0x3A, 0x7B, 0xD5])))
        );
        assert_eq!(
            colour_of(r"%LOCALAPPDATA%\Programs\FolderTintCM\icons\teal.ico"),
            Some(Colour::Tint(Tint::Teal))
        );
        assert_eq!(colour_of(r"C:\FolderTintCM\custom\red.ico"), None);
        assert_eq!(colour_of(r"C:\FolderTintCM\custom\3a7bd5.png"), None);
        assert_eq!(colour_of(r"C:\Other\custom\3a7bd5.ico"), None);
    }

    #[test]
    fn an_ini_with_only_headers_is_empty() {
        assert!(ini_is_empty("[.ShellClassInfo]\r\n\r\n"));
        assert!(ini_is_empty(
            "[.ShellClassInfo]\r\nIconResource=\r\nIconIndex=0\r\n"
        ));
        assert!(ini_is_empty("[.ShellClassInfo]\r\nIconResource=,0\r\n"));
        assert!(!ini_is_empty(
            "[.ShellClassInfo]\r\nIconResource=a.dll,-3\r\n"
        ));
        assert!(!ini_is_empty(
            "[.ShellClassInfo]\r\nLocalizedResourceName=@shell32.dll,-21770\r\n"
        ));
        assert!(!ini_is_empty("[ViewState]\r\nFolderType=Pictures\r\n"));
        assert!(!ini_is_empty(
            "[.ShellClassInfo]\r\nIconResource=C:\\x.ico,0\r\n"
        ));
    }

    #[test]
    fn utf16_ini_is_decoded() {
        let text = "[.ShellClassInfo]\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_ini(&bytes), text);
    }

    /// A temporary folder, removed when the test ends, passed or failed.
    pub(crate) struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.0
        }
    }

    impl AsRef<Path> for Scratch {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // desktop.ini is hidden and system, and the folder read-only.
            for path in [self.0.join("desktop.ini"), self.0.clone()] {
                if let Some(w) = wide(&path) {
                    unsafe {
                        let _ = SetFileAttributesW(PCWSTR(w.as_ptr()), FILE_ATTRIBUTE_NORMAL);
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn scratch(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("foldertintcm-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    /// The real shell call, on a real folder: set, read back, clear, and
    /// nothing left behind.
    #[test]
    fn set_then_clear_round_trip() {
        let dir = scratch("roundtrip");
        let icon = r"%LOCALAPPDATA%\Programs\FolderTintCM\icons\blue.ico";
        assert_eq!(icon_of(&dir), None);

        set_icon(&dir, icon).unwrap();
        let ini = std::fs::read(dir.join("desktop.ini")).expect("desktop.ini written");
        let text = decode_ini(&ini);
        // Written as given, so the folder works for any user who has
        // FolderTintCM; read back expanded.
        assert!(text.contains(icon), "{text}");
        assert!(icon_of(&dir).is_some_and(|i| is_ours(&i) && i.ends_with(r"\icons\blue.ico")));
        let attrs = std::fs::metadata(&dir).unwrap().file_attributes();
        assert!(
            attrs & FILE_ATTRIBUTE_READONLY.0 != 0,
            "folder marked for desktop.ini"
        );

        assert_eq!(clear(&dir).unwrap(), Cleared::Restored);
        assert_eq!(icon_of(&dir), None);
        let left = std::fs::read(dir.join("desktop.ini")).map(|b| decode_ini(&b));
        assert!(left.is_err(), "empty desktop.ini removed: {left:?}");
        let attrs = std::fs::metadata(&dir).unwrap().file_attributes();
        assert_eq!(
            attrs & FILE_ATTRIBUTE_READONLY.0,
            0,
            "read-only mark removed"
        );
    }

    #[test]
    fn clear_leaves_other_customisations_alone() {
        let dir = scratch("foreign");
        let foreign = r"%SystemRoot%\System32\imageres.dll";
        set_icon(&dir, foreign).unwrap();
        assert_eq!(clear(&dir).unwrap(), Cleared::NotOurs);
        let kept = icon_of(&dir).unwrap_or_default().to_ascii_lowercase();
        assert!(kept.ends_with(r"\system32\imageres.dll"), "{kept}");
    }

    #[test]
    fn other_entries_in_desktop_ini_survive() {
        let dir = scratch("keep");
        std::fs::write(
            dir.join("desktop.ini"),
            "[.ShellClassInfo]\r\nInfoTip=keep me\r\n",
        )
        .unwrap();
        set_icon(&dir, r"C:\x\FolderTintCM\icons\red.ico").unwrap();
        clear(&dir).unwrap();
        let text = decode_ini(&std::fs::read(dir.join("desktop.ini")).unwrap());
        assert!(text.contains("InfoTip=keep me"), "{text}");
        assert!(!text.to_ascii_lowercase().contains("red.ico"), "{text}");
    }

    #[test]
    fn files_and_missing_paths_are_refused() {
        let dir = scratch("refuse");
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(
            set_icon(&file, "x.ico"),
            Err(Error::NotAFolder(_))
        ));
        assert!(matches!(
            set_icon(&dir.join("nope"), "x.ico"),
            Err(Error::NotAFolder(_))
        ));
        assert!(matches!(clear(&file), Err(Error::NotAFolder(_))));
    }

    #[test]
    fn drive_roots_are_refused() {
        for root in [r"C:\", r"\\server\share\"] {
            assert!(matches!(
                set_icon(Path::new(root), "x.ico"),
                Err(Error::DriveRoot(_))
            ));
        }
        let dir = scratch("root");
        assert!(colourable(&dir).is_ok());
    }

    fn icon_of(dir: &Path) -> Option<String> {
        icon(dir).unwrap()
    }
}
