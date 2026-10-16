//! What the MSI is built from, generated from the palette so a colour is
//! added in one place: the `.ico` of every tint, and a WiX fragment with
//! the files and the context menu's registry entries.
//!
//! The menu is a classic cascading verb under `Directory` (right click on a
//! folder) and `Directory\Background` (right click inside one), registered
//! per user. Its labels are indirect strings (`@foldertint.exe,-id`) so
//! Explorer shows them in Windows' display language.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::palette::{Tint, strings};
use crate::{art, ico};

const ICONS_GUID: &str = "4129EBE1-5B73-4EF5-93AB-8E2151F70FAD";
const FOLDER_MENU_GUID: &str = "0D416BF2-05DD-4F41-B419-670DB2962D6F";
const BACKGROUND_MENU_GUID: &str = "ED0C4798-B5B7-45F5-810F-AC2A5F0B6228";

/// Writes `<dir>\icons\<tint>.ico` and `<dir>\ContextMenu.wxs`.
pub fn export(dir: &Path) -> io::Result<()> {
    let icons = dir.join("icons");
    std::fs::create_dir_all(&icons)?;
    for tint in Tint::ALL {
        let rgb = art::unit_rgb(tint.rgb());
        let bytes = ico::ico(|size| art::folder(size, |_| rgb));
        std::fs::write(icons.join(format!("{}.ico", tint.slug())), bytes)?;
    }
    std::fs::write(dir.join("ContextMenu.wxs"), context_menu_wxs())
}

/// The brand as PNGs, for the website and later the store listing, drawn
/// from the same code as the program: `icon-<n>.png` (the spectrum
/// folder) and `folders\<tint>.png`, each at 256 pixels.
pub fn export_brand(dir: &Path) -> io::Result<()> {
    let folders = dir.join("folders");
    std::fs::create_dir_all(&folders)?;
    for size in [44u32, 150, 256] {
        let png = ico::png(size, size, &art::spectrum(size));
        std::fs::write(dir.join(format!("icon-{size}.png")), png)?;
    }
    for tint in Tint::ALL {
        let rgb = art::unit_rgb(tint.rgb());
        let png = ico::png(256, 256, &art::folder(256, |_| rgb));
        std::fs::write(folders.join(format!("{}.png", tint.slug())), png)?;
    }
    Ok(())
}

/// Where the verb lives, and how Explorer names the folder in its command.
/// `custom.rs` adds the recent colours under the same keys.
pub struct Place {
    id: &'static str,
    guid: &'static str,
    pub key: &'static str,
    pub folder_arg: &'static str,
}

pub const PLACES: [Place; 2] = [
    Place {
        id: "FolderMenu",
        guid: FOLDER_MENU_GUID,
        key: r"Software\Classes\Directory\shell\FolderTint",
        folder_arg: "%1",
    },
    Place {
        id: "BackgroundMenu",
        guid: BACKGROUND_MENU_GUID,
        key: r"Software\Classes\Directory\Background\shell\FolderTint",
        folder_arg: "%V",
    },
];

/// Shown with a line above it, after the colours.
const ECF_SEPARATORBEFORE: u32 = 0x20;

pub fn context_menu_wxs() -> String {
    let exe = "[INSTALLFOLDER]foldertint.exe";
    let mut x = String::new();
    x.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- Generated from src/palette.rs by foldertint.exe (export-installer-assets). Do not edit. -->\n\
         <Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\">\n  <Fragment>\n",
    );

    x.push_str("    <ComponentGroup Id=\"Tints\" Directory=\"ICONSFOLDER\">\n");
    let _ = writeln!(
        x,
        "      <Component Id=\"TintIcons\" Guid=\"{ICONS_GUID}\">"
    );
    for tint in Tint::ALL {
        let slug = tint.slug();
        let _ = writeln!(
            x,
            "        <File Id=\"Icon_{slug}\" Source=\"$(var.Payload)\\icons\\{slug}.ico\" />"
        );
    }
    x.push_str(
        "        <RegistryValue Root=\"HKCU\" Key=\"Software\\FolderTint\" Name=\"Icons\" Type=\"integer\" Value=\"1\" KeyPath=\"yes\" />\n\
         \x20       <RemoveFolder Id=\"RemoveIconsFolder\" On=\"uninstall\" />\n      </Component>\n    </ComponentGroup>\n\n",
    );

    x.push_str("    <ComponentGroup Id=\"ContextMenu\" Directory=\"INSTALLFOLDER\">\n");
    for place in &PLACES {
        let (key, arg) = (place.key, place.folder_arg);
        let _ = writeln!(
            x,
            "      <Component Id=\"{}\" Guid=\"{}\">",
            place.id, place.guid
        );
        let _ = writeln!(x, "        <RegistryKey Root=\"HKCU\" Key=\"{key}\">");
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"MUIVerb\" Type=\"string\" Value=\"@{exe},-{}\" KeyPath=\"yes\" />",
            strings::MENU
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"Icon\" Type=\"string\" Value=\"{exe},0\" />"
        );
        x.push_str("          <RegistryValue Name=\"SubCommands\" Type=\"string\" Value=\"\" />\n");
        x.push_str("        </RegistryKey>\n");
        for tint in Tint::ALL {
            let slug = tint.slug();
            // Explorer sorts the sub-verbs by key name: the number keeps
            // the palette's order.
            let sub = format!("{key}\\shell\\{:02}{slug}", tint.index() + 1);
            let _ = writeln!(x, "        <RegistryKey Root=\"HKCU\" Key=\"{sub}\">");
            let _ = writeln!(
                x,
                "          <RegistryValue Name=\"MUIVerb\" Type=\"string\" Value=\"@{exe},-{}\" />",
                strings::tint(tint)
            );
            let _ = writeln!(
                x,
                "          <RegistryValue Name=\"Icon\" Type=\"string\" Value=\"[ICONSFOLDER]{slug}.ico\" />"
            );
            let _ = writeln!(
                x,
                "          <RegistryValue Key=\"command\" Type=\"string\" Value=\"&quot;{exe}&quot; --apply {slug} &quot;{arg}&quot;\" />"
            );
            x.push_str("        </RegistryKey>\n");
        }
        // Any other colour: Windows' colour dialog. The recent ones it
        // leaves ("50" to "55") are written by the program, not here.
        let _ = writeln!(
            x,
            "        <RegistryKey Root=\"HKCU\" Key=\"{key}\\shell\\14custom\">"
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"MUIVerb\" Type=\"string\" Value=\"@{exe},-{}\" />",
            strings::CUSTOM
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"Icon\" Type=\"string\" Value=\"{exe},0\" />"
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Key=\"command\" Type=\"string\" Value=\"&quot;{exe}&quot; --pick &quot;{arg}&quot;\" />"
        );
        x.push_str("        </RegistryKey>\n");
        let _ = writeln!(
            x,
            "        <RegistryKey Root=\"HKCU\" Key=\"{key}\\shell\\99clear\">"
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"MUIVerb\" Type=\"string\" Value=\"@{exe},-{}\" />",
            strings::CLEAR
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Name=\"CommandFlags\" Type=\"integer\" Value=\"{ECF_SEPARATORBEFORE}\" />"
        );
        let _ = writeln!(
            x,
            "          <RegistryValue Key=\"command\" Type=\"string\" Value=\"&quot;{exe}&quot; --clear &quot;{arg}&quot;\" />"
        );
        x.push_str("        </RegistryKey>\n      </Component>\n");
    }
    x.push_str("    </ComponentGroup>\n  </Fragment>\n</Wix>\n");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tint_has_an_icon_and_a_verb_in_both_places() {
        let wxs = context_menu_wxs();
        for tint in Tint::ALL {
            let slug = tint.slug();
            assert!(
                wxs.contains(&format!("icons\\{slug}.ico\" />")),
                "{slug} icon file"
            );
            assert_eq!(
                wxs.matches(&format!("--apply {slug} &quot;%1&quot;"))
                    .count(),
                1,
                "{slug} on folders"
            );
            assert_eq!(
                wxs.matches(&format!("--apply {slug} &quot;%V&quot;"))
                    .count(),
                1,
                "{slug} in background"
            );
        }
        assert_eq!(wxs.matches("--clear").count(), 2);
        assert_eq!(wxs.matches("--pick").count(), 2);
    }

    #[test]
    fn verbs_sort_in_palette_order_with_clear_last() {
        let wxs = context_menu_wxs();
        let first = PLACES[0].key;
        let keys: Vec<&str> = wxs
            .lines()
            .filter_map(|l| l.trim().strip_prefix("<RegistryKey Root=\"HKCU\" Key=\""))
            .filter_map(|l| {
                l.strip_prefix(first)
                    .and_then(|k| k.strip_prefix("\\shell\\"))
            })
            .map(|k| k.trim_end_matches("\">"))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert_eq!(keys.len(), Tint::ALL.len() + 2);
        assert_eq!(keys[Tint::ALL.len()], "14custom");
        assert_eq!(keys.last(), Some(&"99clear"));
    }

    #[test]
    fn fragment_is_well_formed_enough() {
        let wxs = context_menu_wxs();
        assert_eq!(
            wxs.matches("<RegistryKey ").count(),
            wxs.matches("</RegistryKey>").count()
        );
        assert_eq!(
            wxs.matches("<Component ").count(),
            wxs.matches("</Component>").count()
        );
        assert!(
            !wxs.contains("--export"),
            "a comment may not hold a double hyphen"
        );
    }

    #[test]
    fn export_writes_every_icon() {
        let dir = std::env::temp_dir().join(format!("foldertint-export-{}", std::process::id()));
        export(&dir).unwrap();
        for tint in Tint::ALL {
            let ico =
                std::fs::read(dir.join("icons").join(format!("{}.ico", tint.slug()))).unwrap();
            assert_eq!(&ico[..4], &[0, 0, 1, 0]);
        }
        assert!(dir.join("ContextMenu.wxs").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
