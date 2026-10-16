//! What the FolderTintCM submenu holds and what each entry runs. Pure: the
//! COM side (`lib.rs`) only turns these into `IExplorerCommand`s.
//!
//! The same entries as the MSI's menu: the palette, "Custom…", the recent
//! colours (the program keeps them in `recent.txt` in its data folder) and
//! "Remove colour". Every entry runs `foldertintcm.exe` with the same
//! command line the MSI's registry entries do.

use std::path::Path;

use crate::palette::{Rgb, Tint};

/// How many recent colours the menu offers, as the program keeps them.
pub const MAX_RECENT: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// `--apply <slug|#rrggbb>`
    Apply(String),
    /// `--pick`
    Pick,
    /// `--clear`
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// An icon resource ("path" or "path,index"); empty for none.
    pub icon: String,
    pub action: Action,
    pub separator_before: bool,
}

/// The submenu, in order. `package` is the package's folder (the program,
/// its icon and the palette's icons), `data` the program's data folder.
pub fn entries(spanish: bool, package: &Path, data: Option<&Path>, recent: &[Rgb]) -> Vec<Entry> {
    let pick = |(es, en): (&'static str, &'static str)| if spanish { es } else { en };
    let exe_icon = format!("{},0", package.join("foldertintcm.exe").display());
    let mut out: Vec<Entry> = Tint::ALL
        .iter()
        .map(|&t| Entry {
            title: pick(t.names()).to_owned(),
            icon: package
                .join("icons")
                .join(format!("{}.ico", t.slug()))
                .display()
                .to_string(),
            action: Action::Apply(t.slug().to_owned()),
            separator_before: false,
        })
        .collect();
    out.push(Entry {
        title: pick(("Personalizado…", "Custom…")).to_owned(),
        icon: exe_icon,
        action: Action::Pick,
        separator_before: false,
    });
    for (i, rgb) in recent.iter().take(MAX_RECENT).enumerate() {
        out.push(Entry {
            title: rgb.label(),
            icon: data
                .map(|d| {
                    d.join("custom")
                        .join(format!("{}.ico", rgb.hex()))
                        .display()
                        .to_string()
                })
                .unwrap_or_default(),
            action: Action::Apply(format!("#{}", rgb.hex())),
            separator_before: i == 0,
        });
    }
    out.push(Entry {
        title: pick(("Quitar color", "Remove colour")).to_owned(),
        icon: String::new(),
        action: Action::Clear,
        separator_before: true,
    });
    out
}

/// `recent.txt`: hex codes separated by commas, newest first.
pub fn parse_recent(text: &str) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::new();
    for rgb in text
        .split(',')
        .filter_map(|h| Rgb::parse(&format!("#{}", h.trim())))
    {
        if !out.contains(&rgb) && out.len() < MAX_RECENT {
            out.push(rgb);
        }
    }
    out
}

/// One argument, quoted the way `CommandLineToArgvW` and Rust's `args()`
/// read it back: inside quotes, backslashes are literal except before a
/// quote, where they must be doubled — so `C:\` ends as `"C:\\"`.
pub fn quote(arg: &str) -> String {
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

/// The command line that runs `action` on `folder`.
pub fn command_line(exe: &Path, action: &Action, folder: &str) -> String {
    let exe = quote(&exe.display().to_string());
    let folder = quote(folder);
    match action {
        Action::Apply(colour) => format!("{exe} --apply {colour} {folder}"),
        Action::Pick => format!("{exe} --pick {folder}"),
        Action::Clear => format!("{exe} --clear {folder}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn palette_then_custom_then_recent_then_clear() {
        let pkg = PathBuf::from(r"C:\Program Files\WindowsApps\Pkg");
        let data = PathBuf::from(r"C:\Users\x\AppData\Local\Packages\F\LocalCache\FolderTintCM");
        let recent = [Rgb([0x3A, 0xC8, 0x78]), Rgb([1, 2, 3])];
        let e = entries(true, &pkg, Some(&data), &recent);
        assert_eq!(e.len(), Tint::ALL.len() + 1 + recent.len() + 1);
        assert_eq!(e[0].title, "Rojo");
        assert_eq!(e[0].icon, r"C:\Program Files\WindowsApps\Pkg\icons\red.ico");
        assert_eq!(e[0].action, Action::Apply("red".into()));
        assert_eq!(e[11].title, "Marrón");
        assert_eq!(e[13].action, Action::Pick);
        assert_eq!(
            e[13].icon,
            r"C:\Program Files\WindowsApps\Pkg\foldertintcm.exe,0"
        );
        assert_eq!(e[14].title, "#3AC878");
        assert!(e[14].separator_before && !e[15].separator_before);
        assert!(e[14].icon.ends_with(r"FolderTintCM\custom\3ac878.ico"));
        let last = e.last().unwrap();
        assert_eq!(
            (last.action.clone(), last.separator_before),
            (Action::Clear, true)
        );
        assert_eq!(entries(false, &pkg, None, &[])[13].title, "Custom…");
    }

    #[test]
    fn recent_file_is_read_like_the_program_writes_it() {
        assert_eq!(
            parse_recent("3ac878,010203\n"),
            vec![Rgb([0x3A, 0xC8, 0x78]), Rgb([1, 2, 3])]
        );
        assert_eq!(parse_recent(""), vec![]);
        assert_eq!(parse_recent("zz,3ac878,3ac878").len(), 1);
        let eight = "aaaa01,aaaa02,aaaa03,aaaa04,aaaa05,aaaa06,aaaa07,aaaa08";
        assert_eq!(parse_recent(eight).len(), MAX_RECENT);
    }

    #[test]
    fn quoting_survives_spaces_quotes_and_trailing_backslashes() {
        assert_eq!(quote(r"C:\Mis cosas"), r#""C:\Mis cosas""#);
        assert_eq!(quote(r"C:\"), r#""C:\\""#);
        assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote(r"a\\b"), r#""a\\b""#);
    }

    #[test]
    fn command_lines_match_the_msi_menu() {
        let exe = PathBuf::from(r"C:\P\foldertintcm.exe");
        assert_eq!(
            command_line(&exe, &Action::Apply("blue".into()), r"D:\Fotos 2026"),
            r#""C:\P\foldertintcm.exe" --apply blue "D:\Fotos 2026""#
        );
        assert_eq!(
            command_line(&exe, &Action::Pick, r"E:\"),
            r#""C:\P\foldertintcm.exe" --pick "E:\\""#
        );
        assert_eq!(
            command_line(&exe, &Action::Clear, "x"),
            r#""C:\P\foldertintcm.exe" --clear "x""#
        );
    }
}
