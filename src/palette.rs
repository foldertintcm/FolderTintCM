//! The colours a folder can take: Apple's system palette (light
//! appearance), in the order macOS lists them. This is the single source of
//! truth — the icons, the context menu, its string resources and the
//! installer are all generated from `Tint::ALL`.
//!
//! Pure data, no Windows calls: `build.rs` includes this file too.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Red,
    Orange,
    Yellow,
    Green,
    Mint,
    Teal,
    Cyan,
    Blue,
    Indigo,
    Purple,
    Pink,
    Brown,
    Gray,
}

impl Tint {
    pub const ALL: [Tint; 13] = [
        Tint::Red,
        Tint::Orange,
        Tint::Yellow,
        Tint::Green,
        Tint::Mint,
        Tint::Teal,
        Tint::Cyan,
        Tint::Blue,
        Tint::Indigo,
        Tint::Purple,
        Tint::Pink,
        Tint::Brown,
        Tint::Gray,
    ];

    /// Stable identifier: the command-line argument, the icon's file name
    /// and the context-menu key. Never rename one — folders already
    /// coloured point at `<slug>.ico`.
    pub const fn slug(self) -> &'static str {
        match self {
            Tint::Red => "red",
            Tint::Orange => "orange",
            Tint::Yellow => "yellow",
            Tint::Green => "green",
            Tint::Mint => "mint",
            Tint::Teal => "teal",
            Tint::Cyan => "cyan",
            Tint::Blue => "blue",
            Tint::Indigo => "indigo",
            Tint::Purple => "purple",
            Tint::Pink => "pink",
            Tint::Brown => "brown",
            Tint::Gray => "gray",
        }
    }

    /// sRGB, as Apple publishes it.
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            Tint::Red => [0xFF, 0x3B, 0x30],
            Tint::Orange => [0xFF, 0x95, 0x00],
            Tint::Yellow => [0xFF, 0xCC, 0x00],
            Tint::Green => [0x34, 0xC7, 0x59],
            Tint::Mint => [0x00, 0xC7, 0xBE],
            Tint::Teal => [0x30, 0xB0, 0xC7],
            Tint::Cyan => [0x32, 0xAD, 0xE6],
            Tint::Blue => [0x00, 0x7A, 0xFF],
            Tint::Indigo => [0x58, 0x56, 0xD6],
            Tint::Purple => [0xAF, 0x52, 0xDE],
            Tint::Pink => [0xFF, 0x2D, 0x55],
            Tint::Brown => [0xA2, 0x84, 0x5E],
            Tint::Gray => [0x8E, 0x8E, 0x93],
        }
    }

    /// (Spanish, English) names, as the context menu shows them. Read by
    /// `build.rs`, which writes them into the string tables.
    #[allow(dead_code)]
    pub const fn names(self) -> (&'static str, &'static str) {
        match self {
            Tint::Red => ("Rojo", "Red"),
            Tint::Orange => ("Naranja", "Orange"),
            Tint::Yellow => ("Amarillo", "Yellow"),
            Tint::Green => ("Verde", "Green"),
            Tint::Mint => ("Menta", "Mint"),
            Tint::Teal => ("Turquesa", "Teal"),
            Tint::Cyan => ("Celeste", "Cyan"),
            Tint::Blue => ("Azul", "Blue"),
            Tint::Indigo => ("Índigo", "Indigo"),
            Tint::Purple => ("Morado", "Purple"),
            Tint::Pink => ("Rosa", "Pink"),
            Tint::Brown => ("Marrón", "Brown"),
            Tint::Gray => ("Gris", "Gray"),
        }
    }

    /// Position in `ALL`, which orders the menu.
    pub fn index(self) -> usize {
        Tint::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    /// Exact, case-sensitive: the argument comes from our own registry
    /// entries, so anything else is refused rather than guessed at.
    pub fn from_slug(slug: &str) -> Option<Tint> {
        Tint::ALL.into_iter().find(|t| t.slug() == slug)
    }
}

/// Any colour the user picks or creates, beyond the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub [u8; 3]);

impl Rgb {
    /// `#rrggbb`, either case; nothing else.
    pub fn parse(s: &str) -> Option<Rgb> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Rgb([byte(0)?, byte(2)?, byte(4)?]))
    }

    /// `rrggbb`: the icon's file name and the stored form.
    pub fn hex(self) -> String {
        let [r, g, b] = self.0;
        format!("{r:02x}{g:02x}{b:02x}")
    }

    /// `#RRGGBB`, as the menu shows it.
    pub fn label(self) -> String {
        format!("#{}", self.hex().to_ascii_uppercase())
    }
}

/// What a folder can be coloured with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Tint(Tint),
    Custom(Rgb),
}

impl Colour {
    /// A palette slug (`blue`) or a custom colour (`#3a7bd5`).
    /// A palette colour when `rgb` is exactly one (its installed icon, no
    /// duplicate among the recent colours), a custom one otherwise.
    pub fn from_rgb(rgb: Rgb) -> Colour {
        Tint::ALL
            .into_iter()
            .find(|t| t.rgb() == rgb.0)
            .map_or(Colour::Custom(rgb), Colour::Tint)
    }

    pub fn parse(s: &str) -> Option<Colour> {
        Tint::from_slug(s)
            .map(Colour::Tint)
            .or_else(|| Rgb::parse(s).map(Colour::Custom))
    }

    pub fn rgb(self) -> [u8; 3] {
        match self {
            Colour::Tint(t) => t.rgb(),
            Colour::Custom(c) => c.0,
        }
    }

    /// As the command line takes it back.
    pub fn arg(self) -> String {
        match self {
            Colour::Tint(t) => t.slug().to_owned(),
            Colour::Custom(c) => format!("#{}", c.hex()),
        }
    }
}

/// String-resource ids in the executable, read by the context menu through
/// `MUIVerb=@foldertint.exe,-<id>` so it follows Windows' display language.
pub mod strings {
    use super::Tint;

    pub const MENU: u16 = 100;
    pub const CUSTOM: u16 = 150;
    pub const CLEAR: u16 = 199;

    pub fn tint(t: Tint) -> u16 {
        101 + t.index() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip_and_are_unique() {
        for t in Tint::ALL {
            assert_eq!(Tint::from_slug(t.slug()), Some(t));
            assert_eq!(Tint::ALL.iter().filter(|o| o.slug() == t.slug()).count(), 1);
        }
    }

    #[test]
    fn unknown_or_differently_cased_slugs_are_refused() {
        assert_eq!(Tint::from_slug("Red"), None);
        assert_eq!(Tint::from_slug(""), None);
        assert_eq!(Tint::from_slug("red\\..\\x"), None);
    }

    #[test]
    fn slugs_are_safe_file_and_key_names() {
        for t in Tint::ALL {
            assert!(t.slug().bytes().all(|b| b.is_ascii_lowercase()));
        }
    }

    #[test]
    fn custom_colours_parse_strictly() {
        assert_eq!(Rgb::parse("#3A7bD5"), Some(Rgb([0x3A, 0x7B, 0xD5])));
        for bad in [
            "3a7bd5", "#3a7bd", "#3a7bd55", "#3a7bzz", "#+1+1+1", "", "#",
        ] {
            assert_eq!(Rgb::parse(bad), None, "{bad}");
        }
        let c = Rgb([0x3A, 0x7B, 0xD5]);
        assert_eq!(c.hex(), "3a7bd5");
        assert_eq!(c.label(), "#3A7BD5");
    }

    #[test]
    fn picked_palette_colours_stay_palette_colours() {
        assert_eq!(
            Colour::from_rgb(Rgb([0x34, 0xC7, 0x59])),
            Colour::Tint(Tint::Green)
        );
        assert_eq!(
            Colour::from_rgb(Rgb([0x34, 0xC7, 0x5A])),
            Colour::Custom(Rgb([0x34, 0xC7, 0x5A]))
        );
    }

    #[test]
    fn colours_round_trip_through_the_command_line() {
        for c in [Colour::Tint(Tint::Brown), Colour::Custom(Rgb([1, 2, 255]))] {
            assert_eq!(Colour::parse(&c.arg()), Some(c));
        }
        assert_eq!(Colour::parse("Brown"), None);
    }

    #[test]
    fn string_ids_stay_between_menu_and_clear() {
        for t in Tint::ALL {
            let id = strings::tint(t);
            assert!(id > strings::MENU && id < strings::CLEAR);
        }
    }
}
