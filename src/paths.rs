//! Where the icons are, and how a folder's `desktop.ini` names them.

use std::io;
use std::path::{Path, PathBuf};

use crate::palette::{Rgb, Tint};

/// The folder the executable runs from; the icons sit in `icons\` beside it.
pub fn install_dir() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "executable has no folder"))
}

pub fn icon_file(install_dir: &Path, tint: Tint) -> PathBuf {
    install_dir
        .join("icons")
        .join(format!("{}.ico", tint.slug()))
}

/// The MSIX package the program runs from (the Microsoft Store build), as
/// its family name; `None` for the MSI.
pub fn package_family() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    crate::package::containing(&exe).map(|(_, family)| family)
}

/// What FolderTintCM keeps per user, apart from the program:
/// `%LOCALAPPDATA%\FolderTintCM` for the MSI (its uninstaller removes it),
/// the package's own data folder for the Store build (Windows removes it
/// with the package, and the folders coloured from it go back to normal).
pub fn data_dir() -> io::Result<PathBuf> {
    let missing = || io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set");
    match package_family() {
        Some(family) => crate::package::local_cache(&family)
            .map(|dir| dir.join("FolderTintCM"))
            .ok_or_else(missing),
        None => std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("FolderTintCM"))
            .ok_or_else(missing),
    }
}

/// A palette colour's icon as a coloured folder names it. The MSI's own
/// copy beside the program; for the Store build, a copy in the data folder,
/// since the package's folder changes name with every update.
pub fn tint_icon(data_dir: &Path, tint: Tint) -> PathBuf {
    data_dir.join("icons").join(format!("{}.ico", tint.slug()))
}

/// The icon of a colour the user picked, drawn on first use.
pub fn custom_icon(data_dir: &Path, rgb: Rgb) -> PathBuf {
    data_dir.join("custom").join(format!("{}.ico", rgb.hex()))
}

/// The icon's path as `desktop.ini` should hold it: under
/// `%LOCALAPPDATA%` when FolderTintCM is installed there (the per-user MSI
/// puts it there), so a coloured folder on a shared or removable drive
/// shows its colour for every user who has FolderTintCM, not only for the
/// one who coloured it.
pub fn portable(icon: &Path) -> String {
    let icon = icon.display().to_string();
    match std::env::var("LOCALAPPDATA") {
        Ok(base) => with_variable(&icon, "LOCALAPPDATA", &base),
        Err(_) => icon,
    }
}

/// `path` with a leading `value` replaced by `%name%`, compared without
/// regard to case as Windows paths are; whole path components only.
fn with_variable(path: &str, name: &str, value: &str) -> String {
    let value = value.trim_end_matches('\\');
    if value.is_empty() || path.len() <= value.len() {
        return path.to_owned();
    }
    let (head, tail) = path.split_at(value.len());
    if head.eq_ignore_ascii_case(value) && tail.starts_with('\\') {
        format!("%{name}%{tail}")
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_leading_folder_only() {
        let base = r"C:\Users\Ana\AppData\Local";
        assert_eq!(
            with_variable(
                r"C:\Users\Ana\AppData\Local\Programs\FolderTintCM\icons\red.ico",
                "LOCALAPPDATA",
                base
            ),
            r"%LOCALAPPDATA%\Programs\FolderTintCM\icons\red.ico"
        );
        assert_eq!(
            with_variable(
                r"c:\users\ana\appdata\local\x.ico",
                "LOCALAPPDATA",
                &format!(r"{base}\")
            ),
            r"%LOCALAPPDATA%\x.ico"
        );
        // A sibling whose name merely starts the same is not inside it.
        assert_eq!(
            with_variable(r"C:\Users\Ana\AppData\LocalLow\x.ico", "LOCALAPPDATA", base),
            r"C:\Users\Ana\AppData\LocalLow\x.ico"
        );
        assert_eq!(
            with_variable(r"D:\Tools\x.ico", "LOCALAPPDATA", base),
            r"D:\Tools\x.ico"
        );
        assert_eq!(with_variable(r"D:\x.ico", "LOCALAPPDATA", ""), r"D:\x.ico");
    }

    #[test]
    fn icons_are_named_by_slug() {
        assert_eq!(
            icon_file(Path::new(r"C:\FT"), Tint::Mint),
            PathBuf::from(r"C:\FT\icons\mint.ico")
        );
    }
}
