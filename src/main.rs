//! FolderTint: colours folders from Explorer's context menu.
//!
//! There is no resident process. Each menu entry runs this executable once
//! (`--apply <colour> <folder>`, `--pick <folder>` or `--clear <folder>`),
//! which sets the folder's icon through the shell and exits; the installer
//! registers the menu and ships one `.ico` per palette colour beside the
//! executable. Colours picked in the dialog get their icon drawn on first
//! use, under `%LOCALAPPDATA%\FolderTint\custom`.

#![windows_subsystem = "windows"]

mod art;
mod cli;
mod custom;
mod folder;
mod i18n;
mod ico;
mod installer;
mod ledger;
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
                "Uso: foldertint --apply <color|#rrggbb> <carpeta> | --pick <carpeta> | --clear <carpeta>",
                "Usage: foldertint --apply <colour|#rrggbb> <folder> | --pick <folder> | --clear <folder>",
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
    let exe_dir = paths::install_dir().map_err(|e| e.to_string())?;
    let icon = match colour {
        Colour::Tint(tint) => paths::icon_file(&exe_dir, tint),
        Colour::Custom(rgb) => {
            let data = paths::data_dir().map_err(|e| e.to_string())?;
            custom::ensure_icon(&data, rgb).map_err(|e| e.to_string())?
        }
    };
    // Pointing a folder at a missing icon would leave it blank.
    if !icon.is_file() {
        return Err(format!(
            "{}\n{}",
            tr(
                "Falta el icono; reinstale FolderTint.",
                "The icon is missing; reinstall FolderTint."
            ),
            icon.display()
        ));
    }
    folder::set_icon(folder, &paths::portable(&icon)).map_err(describe)?;
    // Only for tidying up on uninstall and for the menu: a failure here
    // must not undo the colour the user just chose.
    let _ = Ledger::user().record(folder, colour);
    if let (Colour::Custom(rgb), Ok(exe), Ok(data)) =
        (colour, std::env::current_exe(), paths::data_dir())
    {
        custom::remember(rgb, &exe, &data);
    }
    Ok(())
}

/// The colour dialog, opened on the folder's current colour (or the last
/// one created), then that colour applied. Cancel changes nothing.
fn pick(folder: &Path) -> Result<(), String> {
    let current = folder::icon(folder)
        .map_err(describe)?
        .and_then(|icon| folder::colour_of(&icon))
        .map(|c| Rgb(c.rgb()));
    let initial = current
        .or_else(|| custom::recent().first().copied())
        .unwrap_or(Rgb(Tint::Blue.rgb()));
    match custom::pick_shared(initial) {
        Some(rgb) => apply(Colour::from_rgb(rgb), folder),
        None => Ok(()),
    }
}

fn clear(folder: &Path) -> Result<(), String> {
    folder::clear(folder).map_err(describe)?;
    Ledger::user().forget(folder);
    Ok(())
}

/// Every folder FolderTint coloured gets its normal icon back, then
/// everything FolderTint kept goes: the recent colours in the menu, their
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

fn about() {
    message(
        tr(
            "FolderTint está instalado.\n\nHaga clic derecho en una carpeta y elija «FolderTint». \
             En Windows 11 está en «Mostrar más opciones» (Mayús+F10).",
            "FolderTint is installed.\n\nRight-click a folder and choose \"FolderTint\". \
             On Windows 11 it is under \"Show more options\" (Shift+F10).",
        ),
        MB_ICONINFORMATION,
    );
}

fn fail(text: &str) -> ExitCode {
    message(text, MB_ICONERROR);
    ExitCode::FAILURE
}

fn message(text: &str, icon: MESSAGEBOX_STYLE) {
    let (Some(text), Some(title)) = (wide::wide(text), wide::wide("FolderTint")) else {
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
