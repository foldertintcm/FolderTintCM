//! The command line: what Explorer's context menu, the installer and the
//! build script ask for. Parsing is strict — an unknown flag, colour or
//! extra argument is an error, never a guess.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::palette::Colour;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Started on its own (Start menu, double click): say how it is used.
    About,
    /// `--apply <colour> <folder>`: a palette slug or `#rrggbb`.
    Apply { colour: Colour, folder: PathBuf },
    /// `--pick <folder>`: choose or create a colour in Windows' dialog.
    Pick { folder: PathBuf },
    /// `--clear <folder>`
    Clear { folder: PathBuf },
    /// `--uninstall`: the installer, before it removes the icons, gives
    /// every folder FolderTint coloured its normal icon back.
    Uninstall,
    /// `--export-installer-assets <dir>`: the icons and the WiX fragment
    /// the MSI is built from.
    ExportInstallerAssets { dir: PathBuf },
    /// `--export-brand <dir>`: the icon and the folders as PNGs, for the
    /// website.
    ExportBrand { dir: PathBuf },
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliError {
    UnknownColour(String),
    Usage,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, CliError> {
    let args: Vec<OsString> = args.into_iter().collect();
    let flag = args.first().and_then(|a| a.to_str());
    let rest = &args[args.len().min(1)..];
    match (flag, rest) {
        (None, []) => Ok(Command::About),
        (Some("--apply"), [colour, folder]) => {
            let text = colour.to_string_lossy();
            let colour =
                Colour::parse(&text).ok_or_else(|| CliError::UnknownColour(text.into()))?;
            Ok(Command::Apply {
                colour,
                folder: non_empty(folder)?,
            })
        }
        (Some("--pick"), [folder]) => Ok(Command::Pick {
            folder: non_empty(folder)?,
        }),
        (Some("--clear"), [folder]) => Ok(Command::Clear {
            folder: non_empty(folder)?,
        }),
        (Some("--uninstall"), []) => Ok(Command::Uninstall),
        (Some("--export-brand"), [dir]) => Ok(Command::ExportBrand {
            dir: non_empty(dir)?,
        }),
        (Some("--export-installer-assets"), [dir]) => Ok(Command::ExportInstallerAssets {
            dir: non_empty(dir)?,
        }),
        _ => Err(CliError::Usage),
    }
}

fn non_empty(arg: &OsString) -> Result<PathBuf, CliError> {
    if arg.is_empty() {
        Err(CliError::Usage)
    } else {
        Ok(PathBuf::from(arg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Rgb, Tint};

    fn p(args: &[&str]) -> Result<Command, CliError> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_is_about() {
        assert_eq!(p(&[]), Ok(Command::About));
    }

    #[test]
    fn apply_takes_a_known_colour_and_a_folder() {
        assert_eq!(
            p(&["--apply", "blue", r"C:\Users\x\Mis cosas"]),
            Ok(Command::Apply {
                colour: Colour::Tint(Tint::Blue),
                folder: r"C:\Users\x\Mis cosas".into()
            })
        );
        assert_eq!(
            p(&["--apply", "#3A7BD5", r"C:\x"]),
            Ok(Command::Apply {
                colour: Colour::Custom(Rgb([0x3A, 0x7B, 0xD5])),
                folder: r"C:\x".into()
            })
        );
        for bad in ["chartreuse", "3a7bd5", "#3a7bd"] {
            assert_eq!(
                p(&["--apply", bad, r"C:\x"]),
                Err(CliError::UnknownColour(bad.into()))
            );
        }
    }

    #[test]
    fn pick_takes_one_folder() {
        assert_eq!(
            p(&["--pick", r"C:\x"]),
            Ok(Command::Pick {
                folder: r"C:\x".into()
            })
        );
        assert_eq!(p(&["--pick"]), Err(CliError::Usage));
        assert_eq!(p(&["--pick", ""]), Err(CliError::Usage));
    }

    #[test]
    fn wrong_shapes_are_usage_errors() {
        assert_eq!(p(&["--apply", "blue"]), Err(CliError::Usage));
        assert_eq!(p(&["--apply", "blue", ""]), Err(CliError::Usage));
        assert_eq!(p(&["--clear"]), Err(CliError::Usage));
        assert_eq!(p(&["--clear", r"C:\a", r"C:\b"]), Err(CliError::Usage));
        assert_eq!(p(&["--uninstall", "now"]), Err(CliError::Usage));
        assert_eq!(p(&["--frobnicate"]), Err(CliError::Usage));
        assert_eq!(p(&[r"C:\x"]), Err(CliError::Usage));
    }

    #[test]
    fn clear_uninstall_and_export() {
        assert_eq!(
            p(&["--clear", r"D:\a"]),
            Ok(Command::Clear {
                folder: r"D:\a".into()
            })
        );
        assert_eq!(p(&["--uninstall"]), Ok(Command::Uninstall));
        assert_eq!(
            p(&["--export-installer-assets", "out"]),
            Ok(Command::ExportInstallerAssets { dir: "out".into() })
        );
        assert_eq!(
            p(&["--export-brand", "web"]),
            Ok(Command::ExportBrand { dir: "web".into() })
        );
    }
}
