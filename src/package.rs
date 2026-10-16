//! Whether a file sits inside an installed MSIX package (the Microsoft
//! Store build), and where that package keeps its data.
//!
//! Decided from the path, not from package identity: the menu extension
//! runs inside Explorer, which has no identity, and starts the program
//! without one either. A package is installed in a folder named after its
//! full name (`Name_Version_Arch__PublisherId`), which Windows turns into
//! the family name that names the data folder:
//! `%LOCALAPPDATA%\Packages\<family>\LocalCache`. That folder keeps its
//! path across updates and goes away when the package is uninstalled.
//!
//! Shared with the menu extension (`shellext`), which includes this file.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Storage::Packaging::Appx::PackageFamilyNameFromFullName;
use windows::core::{PCWSTR, PWSTR};

/// The family name of a package's full name, or `None` if `full` is not one.
pub fn family_name(full: &str) -> Option<String> {
    let mut wide: Vec<u16> = full.encode_utf16().collect();
    if wide.contains(&0) || wide.is_empty() {
        return None;
    }
    wide.push(0);
    // PACKAGE_FAMILY_NAME_MAX_LENGTH is 64 + 1 + 13 + NUL.
    let mut buf = [0u16; 128];
    let mut len = buf.len() as u32;
    let e = unsafe {
        PackageFamilyNameFromFullName(
            PCWSTR(wide.as_ptr()),
            &mut len,
            Some(PWSTR(buf.as_mut_ptr())),
        )
    };
    if e != ERROR_SUCCESS {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// The package `path` is installed in: (its folder, its family name).
pub fn containing(path: &Path) -> Option<(PathBuf, String)> {
    path.ancestors().find_map(|dir| {
        let name = dir.file_name()?.to_str()?;
        // A full name always holds the "__" before the publisher id; most
        // folders can be skipped without asking Windows.
        if !name.contains("__") {
            return None;
        }
        family_name(name).map(|family| (dir.to_path_buf(), family))
    })
}

/// The package's own data folder, `%LOCALAPPDATA%\Packages\<family>\LocalCache`.
pub fn local_cache(family: &str) -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join("Packages")
            .join(family)
            .join("LocalCache"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "Microsoft.WindowsCalculator_11.2405.2.0_x64__8wekyb3d8bbwe";

    #[test]
    fn full_names_become_family_names() {
        assert_eq!(
            family_name(FULL).as_deref(),
            Some("Microsoft.WindowsCalculator_8wekyb3d8bbwe")
        );
        assert_eq!(family_name("FolderTintCM"), None);
        assert_eq!(family_name(""), None);
    }

    #[test]
    fn finds_the_package_above_a_file() {
        let exe = PathBuf::from(format!(
            r"C:\Program Files\WindowsApps\{FULL}\foldertintcm.exe"
        ));
        let (dir, family) = containing(&exe).expect("inside a package");
        assert!(dir.ends_with(FULL));
        assert_eq!(family, "Microsoft.WindowsCalculator_8wekyb3d8bbwe");
        let msi = PathBuf::from(r"C:\Users\x\AppData\Local\Programs\FolderTintCM\foldertintcm.exe");
        assert_eq!(containing(&msi), None);
    }

    #[test]
    fn data_lives_under_packages() {
        let dir = local_cache("A.B_8wekyb3d8bbwe").unwrap();
        assert!(dir.ends_with(r"Packages\A.B_8wekyb3d8bbwe\LocalCache"));
    }
}
