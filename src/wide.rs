//! UTF-16 strings for Windows calls.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// NUL-terminated UTF-16. An interior NUL would silently cut the string
/// short at the API boundary, so it is refused instead.
pub fn wide(s: impl AsRef<OsStr>) -> Option<Vec<u16>> {
    let mut v: Vec<u16> = s.as_ref().encode_wide().collect();
    if v.contains(&0) {
        return None;
    }
    v.push(0);
    Some(v)
}

/// The string in a NUL-terminated (or full) UTF-16 buffer.
pub fn from_buffer(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminates_and_refuses_interior_nul() {
        assert_eq!(wide("ab"), Some(vec![97, 98, 0]));
        assert_eq!(wide("a\0b"), None);
        assert_eq!(from_buffer(&[104, 105, 0, 120]), "hi");
        assert_eq!(from_buffer(&[104, 105]), "hi");
    }
}
