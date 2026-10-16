//! The folders FolderTint has coloured, kept in the registry
//! (`HKCU\Software\FolderTint\Folders`: one value per folder, named by its
//! path, holding the colour) so that uninstalling can give each of them its
//! normal icon back instead of leaving it pointing at a deleted file.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
};
use windows::core::{PCWSTR, PWSTR};

use crate::palette::Colour;
use crate::reg;
use crate::wide::{from_buffer, wide};

pub struct Ledger {
    key: String,
}

impl Ledger {
    pub fn user() -> Ledger {
        Ledger::at(r"Software\FolderTint\Folders")
    }

    fn at(key: &str) -> Ledger {
        Ledger {
            key: key.to_owned(),
        }
    }

    pub fn record(&self, folder: &Path, colour: Colour) -> windows::core::Result<()> {
        match folder.to_str() {
            Some(name) => reg::set_string(&self.key, Some(name), &colour.arg()),
            // A path that is not valid Unicode cannot be a value name.
            None => Ok(()),
        }
    }

    pub fn forget(&self, folder: &Path) {
        if let Some(name) = folder.to_str() {
            reg::delete_value(&self.key, name);
        }
    }

    /// Every folder recorded; empty when none (or no key yet).
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Some(path) = wide(&self.key) else {
            return out;
        };
        let mut key = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                None,
                KEY_READ,
                &mut key,
            )
        } != ERROR_SUCCESS
        {
            return out;
        }
        // Value names are at most 16 383 characters.
        let mut name = vec![0u16; 16_384];
        for index in 0.. {
            let mut len = name.len() as u32;
            let e = unsafe {
                RegEnumValueW(
                    key,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            // ERROR_NO_MORE_ITEMS at the end; anything else, stop too.
            if e != ERROR_SUCCESS {
                break;
            }
            out.push(PathBuf::from(from_buffer(&name[..len as usize])));
        }
        unsafe {
            let _ = RegCloseKey(key);
        }
        out
    }

    /// Deletes the list, and the application's key above it once nothing
    /// else is left in it: on uninstall, the installer has removed its own
    /// values by then and would leave the key behind, empty.
    pub fn forget_all(&self) {
        reg::delete_tree(&self.key);
        if let Some((parent, _)) = self.key.rsplit_once('\\') {
            reg::delete_if_empty(parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Rgb, Tint};

    #[test]
    fn forgetting_all_removes_an_empty_parent_only() {
        let parent = format!(r"Software\FolderTint-Test-{}", std::process::id());
        let ledger = Ledger::at(&format!(r"{parent}\Folders"));
        ledger
            .record(Path::new(r"C:\a"), Colour::Tint(Tint::Red))
            .unwrap();
        ledger.forget_all();
        assert!(!reg::exists(&parent), "empty parent removed");

        // A parent holding something else stays.
        ledger
            .record(Path::new(r"C:\a"), Colour::Tint(Tint::Red))
            .unwrap();
        reg::set_string(&parent, Some("keep"), "x").unwrap();
        ledger.forget_all();
        assert!(reg::exists(&parent), "parent with a value kept");
        reg::delete_tree(&parent);
        assert!(!reg::exists(&parent));
    }

    #[test]
    fn records_lists_and_forgets() {
        let parent = format!(r"Software\FolderTint-Test-{}-b", std::process::id());
        let ledger = Ledger::at(&format!(r"{parent}\Folders"));
        ledger.forget_all();
        assert!(ledger.folders().is_empty());

        let a = PathBuf::from(r"C:\Fotos\Viaje à Lisboa");
        let b = PathBuf::from(r"D:\Proyectos");
        ledger.record(&a, Colour::Tint(Tint::Red)).unwrap();
        ledger.record(&b, Colour::Custom(Rgb([1, 2, 3]))).unwrap();
        ledger.record(&a, Colour::Tint(Tint::Green)).unwrap(); // recolouring keeps one entry
        let mut folders = ledger.folders();
        folders.sort();
        assert_eq!(folders, vec![a.clone(), b.clone()]);

        ledger.forget(&a);
        assert_eq!(ledger.folders(), vec![b]);
        ledger.forget_all();
        assert!(ledger.folders().is_empty());
        assert!(!reg::exists(&parent));
    }
}
