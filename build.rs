//! Embeds into the executable, with the resource compiler llvm-mingw ships
//! (`windres`): the application icon, drawn here from `src/art.rs`; the
//! manifest; the version information; and the context menu's labels as
//! string tables in Spanish and English, generated from `src/palette.rs`.

// The drawing and the palette, shared with the program. Pure Rust: no
// Windows calls, so they build for the build script too.
#[allow(dead_code)]
#[path = "src/art.rs"]
mod art;
#[allow(dead_code)]
#[path = "src/ico.rs"]
mod ico;
#[allow(dead_code)]
#[path = "src/palette.rs"]
mod palette;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use palette::{Tint, strings};

/// `LANGUAGE` statements: (primary, sub-language, Spanish?). Neutral
/// sub-languages match every country. No language-neutral (0, 0) table:
/// the loader tries that one first and it hid the Spanish one on an es-ES
/// Windows. Any other language falls back to the lowest id, English.
const TABLES: [(u16, u16, bool); 2] = [(0x0A, 0x00, true), (0x09, 0x00, false)];

/// A wide RC string literal. Everything outside printable ASCII is written
/// as a UTF-16 escape: windres read the file's UTF-8 as Windows-1252 and
/// the menu showed "MarrÃ³n" (seen 2026-10-15), `#pragma code_page`
/// notwithstanding.
fn rc_string(s: &str) -> String {
    let mut out = String::from("L\"");
    for unit in s.encode_utf16() {
        match unit {
            0x22 => out.push_str("\"\""),
            0x5C => out.push_str("\\\\"),
            0x20..=0x7E => out.push(unit as u8 as char),
            _ => {
                let _ = write!(out, "\\x{unit:04x}");
            }
        }
    }
    out.push('"');
    out
}

fn string_tables() -> String {
    let mut rc = String::new();
    for (primary, sub, spanish) in TABLES {
        let pick = |(es, en): (&'static str, &'static str)| if spanish { es } else { en };
        let _ = writeln!(
            rc,
            "LANGUAGE {primary:#04x}, {sub:#04x}\nSTRINGTABLE\nBEGIN"
        );
        let mut entry = |id: u16, text: &str| {
            let _ = writeln!(rc, "  {id} {}", rc_string(text));
        };
        // The product's own name, in every language.
        entry(strings::MENU, "FolderTintCM");
        for tint in Tint::ALL {
            entry(strings::tint(tint), pick(tint.names()));
        }
        entry(strings::CUSTOM, pick(("Personalizado…", "Custom…")));
        entry(strings::CLEAR, pick(("Quitar color", "Remove colour")));
        rc.push_str("END\n\n");
    }
    rc
}

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    let (major, minor, patch) = (
        parts[0],
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    );

    let icon = out.join("foldertintcm.ico");
    fs::write(&icon, ico::ico(art::spectrum)).unwrap();

    let path = |p: PathBuf| p.display().to_string().replace('\\', "/");
    let rc = format!(
        r#"
#include <winver.h>
LANGUAGE 0x09, 0x01
1 ICON "{icon}"
1 24 "{manifest}"
VS_VERSION_INFO VERSIONINFO
 FILEVERSION {major},{minor},{patch},0
 PRODUCTVERSION {major},{minor},{patch},0
 FILEOS VOS_NT_WINDOWS32
 FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "FolderTintCM"
      VALUE "FileDescription", "FolderTintCM - folder colours"
      VALUE "LegalCopyright", "Copyright (C) 2026 cmonroyp. GPL-3.0-or-later."
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "foldertintcm"
      VALUE "OriginalFilename", "foldertintcm.exe"
      VALUE "ProductName", "FolderTintCM"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END

{tables}"#,
        icon = path(icon.clone()),
        manifest = path(manifest_dir.join("assets").join("app.manifest")),
        tables = string_tables(),
    );
    let rc_path = out.join("foldertintcm.rc");
    fs::write(&rc_path, rc).unwrap();
    let obj = out.join("foldertintcm-res.o");
    let status = Command::new("x86_64-w64-mingw32-windres")
        .arg(&rc_path)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status()
        .expect("windres from llvm-mingw must be on PATH (scripts/cargo.ps1 puts it there)");
    assert!(status.success(), "windres failed");
    // Linked into the binary only: tests need no resources.
    println!("cargo:rustc-link-arg-bins={}", obj.display());
    for f in [
        "src/art.rs",
        "src/ico.rs",
        "src/palette.rs",
        "assets/app.manifest",
        "build.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }
}
