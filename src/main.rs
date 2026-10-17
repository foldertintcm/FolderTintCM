//! FolderTintCM: colours folders from Explorer's context menu.
//!
//! There is no resident process. Each menu entry runs this executable once
//! (`--apply <colour> <folder>`, `--pick <folder>` or `--clear <folder>`),
//! which sets the folder's icon through the shell and exits; the installer
//! registers the menu and ships one `.ico` per palette colour beside the
//! executable. Colours picked in the dialog get their icon drawn on first
//! use, under `%LOCALAPPDATA%\FolderTintCM\custom`.

#![windows_subsystem = "windows"]

mod art;
mod cli;
mod custom;
mod folder;
mod i18n;
mod ico;
mod installer;
mod ledger;
mod package;
mod palette;
mod paths;
mod picker;
mod reg;
mod wide;

use std::path::Path;
use std::process::ExitCode;

use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::PCWSTR;

use cli::Command;
use i18n::tr;
use ledger::Ledger;
use palette::{Colour, Rgb, Tint};

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args_os().skip(1)) {
        Ok(c) => c,
        Err(cli::CliError::UnknownColour(c)) => {
            return fail(&format!(
                "{} «{c}».",
                tr("Color desconocido", "Unknown colour")
            ));
        }
        Err(cli::CliError::Usage) => {
            return fail(tr(
                "Uso: foldertintcm --apply <color|#rrggbb> <carpeta> | --pick <carpeta> | --clear <carpeta>",
                "Usage: foldertintcm --apply <colour|#rrggbb> <folder> | --pick <folder> | --clear <folder>",
            ));
        }
    };
    match command {
        Command::About => {
            about();
            ExitCode::SUCCESS
        }
        Command::Apply { colour, folder } => report(apply(colour, &folder)),
        Command::Pick { folder } => report(pick(&folder)),
        Command::Clear { folder } => report(clear(&folder)),
        Command::Uninstall => {
            uninstall();
            ExitCode::SUCCESS
        }
        // Run by the build scripts: no dialog, the exit code says it all.
        Command::ExportInstallerAssets { dir } => exit_code(installer::export(&dir)),
        Command::ExportBrand { dir } => exit_code(installer::export_brand(&dir)),
        Command::ExportPackageAssets { dir } => exit_code(installer::export_package_assets(&dir)),
    }
}

fn exit_code(result: std::io::Result<()>) -> ExitCode {
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn apply(colour: Colour, folder: &Path) -> Result<(), String> {
    let data = paths::data_dir().map_err(|e| e.to_string())?;
    let packaged = paths::package_family().is_some();
    let icon = match colour {
        // The Store build draws its palette icons into its data folder:
        // the package's own folder is renamed by every update.
        Colour::Tint(tint) if packaged => {
            let icon = paths::tint_icon(&data, tint);
            custom::ensure_icon_at(&icon, Rgb(tint.rgb())).map_err(|e| e.to_string())?;
            icon
        }
        Colour::Tint(tint) => {
            let exe_dir = paths::install_dir().map_err(|e| e.to_string())?;
            paths::icon_file(&exe_dir, tint)
        }
        Colour::Custom(rgb) => custom::ensure_icon(&data, rgb).map_err(|e| e.to_string())?,
    };
    // Pointing a folder at a missing icon would leave it blank.
    if !icon.is_file() {
        return Err(format!(
            "{}\n{}",
            tr(
                "Falta el icono; reinstale FolderTintCM.",
                "The icon is missing; reinstall FolderTintCM."
            ),
            icon.display()
        ));
    }
    folder::set_icon(folder, &paths::portable(&icon)).map_err(describe)?;
    // Only for tidying up on uninstall and for the menu: a failure here
    // must not undo the colour the user just chose. The Store build has no
    // uninstall step to run (its icons go with the package, and the folders
    // with them), so it keeps no list in the registry.
    if !packaged {
        let _ = Ledger::user().record(folder, colour);
    }
    if let (Colour::Custom(rgb), Ok(exe)) = (colour, std::env::current_exe()) {
        custom::remember(rgb, &exe, &data);
    }
    folder::renotify(folder);
    Ok(())
}

/// The colour dialog, opened on the folder's current colour (or the last
/// one created), then that colour applied. Cancel changes nothing.
fn pick(folder: &Path) -> Result<(), String> {
    folder::colourable(folder).map_err(describe)?;
    let current = folder::icon(folder)
        .map_err(describe)?
        .and_then(|icon| folder::colour_of(&icon))
        .map(|c| Rgb(c.rgb()));
    let data = paths::data_dir().map_err(|e| e.to_string())?;
    let initial = current
        .or_else(|| custom::recent(&data).first().copied())
        .unwrap_or(Rgb(Tint::Blue.rgb()));
    match custom::pick_shared(initial, &data) {
        Some(rgb) => apply(Colour::from_rgb(rgb), folder),
        None => Ok(()),
    }
}

fn clear(folder: &Path) -> Result<(), String> {
    if folder::clear(folder).map_err(describe)? == folder::Cleared::Restored {
        folder::renotify(folder);
    }
    Ledger::user().forget(folder);
    Ok(())
}

/// Every folder FolderTintCM coloured gets its normal icon back, then
/// everything FolderTintCM kept goes: the recent colours in the menu, their
/// icons and its registry key. Best effort: a folder since deleted, moved
/// or on a drive not plugged in is skipped.
fn uninstall() {
    let ledger = Ledger::user();
    for folder in ledger.folders() {
        let _ = folder::clear(&folder);
    }
    custom::forget_all();
    if let Ok(data) = paths::data_dir() {
        let _ = std::fs::remove_dir_all(data);
    }
    ledger.forget_all();
}

fn describe(e: folder::Error) -> String {
    match e {
        folder::Error::NotAFolder(p) => format!(
            "{}\n{}",
            tr(
                "No es una carpeta o ya no existe:",
                "Not a folder, or it no longer exists:"
            ),
            p.display()
        ),
        folder::Error::DriveRoot(p) => format!(
            "{}\n{}",
            tr(
                "Las unidades no se pueden colorear, solo las carpetas que contienen:",
                "Drives cannot be coloured, only the folders in them:"
            ),
            p.display()
        ),
        other => format!(
            "{}\n{other}",
            tr(
                "Windows no permitió cambiar la carpeta.",
                "Windows did not allow the folder to be changed."
            )
        ),
    }
}

fn report(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => fail(&message),
    }
}

/// Opened from Start: where FolderTintCM lives. The Store build's menu is in
/// Windows 11's first menu; the MSI's, under "Show more options".
fn about() {
    let text = if paths::package_family().is_some() {
        tr(
            "FolderTintCM está instalado.\n\nHaga clic derecho en una carpeta y elija «FolderTintCM» \
             para darle uno de los 13 colores, uno propio con «Personalizado…», o quitarlo.",
            "FolderTintCM is installed.\n\nRight-click a folder and choose \"FolderTintCM\" to give it \
             one of the 13 colours, your own with \"Custom…\", or remove it.",
        )
    } else {
        tr(
            "FolderTintCM está instalado.\n\nHaga clic derecho en una carpeta y elija «FolderTintCM». \
             En Windows 11 está en «Mostrar más opciones» (Mayús+F10).",
            "FolderTintCM is installed.\n\nRight-click a folder and choose \"FolderTintCM\". \
             On Windows 11 it is under \"Show more options\" (Shift+F10).",
        )
    };
    message(text, MB_ICONINFORMATION);
}

fn fail(text: &str) -> ExitCode {
    message(text, MB_ICONERROR);
    ExitCode::FAILURE
}

fn message(text: &str, icon: MESSAGEBOX_STYLE) {
    let (Some(text), Some(title)) = (wide::wide(text), wide::wide("FolderTintCM")) else {
        return;
    };
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | icon,
        );
    }
}
