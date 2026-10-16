//! The few registry operations FolderTint needs, all under
//! `HKEY_CURRENT_USER` (it never asks for elevation), with paths as `&str`.

use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_ROUTINE_FLAGS, REG_SZ, REG_VALUE_TYPE,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegDeleteKeyValueW, RegDeleteKeyW,
    RegDeleteTreeW, RegGetValueW, RegOpenKeyExW, RegQueryInfoKeyW, RegSetKeyValueW,
};
use windows::core::PCWSTR;

use crate::wide::{from_buffer, wide};

fn ok(e: WIN32_ERROR) -> windows::core::Result<()> {
    if e == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(e.into())
    }
}

/// UTF-16 for a key path or value name; `None` (the default value) stays a
/// null pointer.
struct W(Option<Vec<u16>>);

impl W {
    fn new(s: Option<&str>) -> W {
        W(s.and_then(wide))
    }

    fn ptr(&self) -> PCWSTR {
        self.0
            .as_ref()
            .map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr()))
    }
}

fn set(
    key: &str,
    name: Option<&str>,
    kind: REG_VALUE_TYPE,
    data: &[u8],
) -> windows::core::Result<()> {
    let (k, n) = (W::new(Some(key)), W::new(name));
    ok(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            k.ptr(),
            n.ptr(),
            kind.0,
            Some(data.as_ptr().cast()),
            data.len() as u32,
        )
    })
}

fn get(key: &str, name: Option<&str>, kind: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
    let (k, n) = (W::new(Some(key)), W::new(name));
    let mut len = 0u32;
    unsafe {
        if RegGetValueW(
            HKEY_CURRENT_USER,
            k.ptr(),
            n.ptr(),
            kind,
            None,
            None,
            Some(&mut len),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        if RegGetValueW(
            HKEY_CURRENT_USER,
            k.ptr(),
            n.ptr(),
            kind,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        buf.truncate(len as usize);
        Some(buf)
    }
}

pub fn set_string(key: &str, name: Option<&str>, value: &str) -> windows::core::Result<()> {
    let data = wide(value).unwrap_or_else(|| vec![0]);
    let bytes: Vec<u8> = data.iter().flat_map(|u| u.to_le_bytes()).collect();
    set(key, name, REG_SZ, &bytes)
}

pub fn get_string(key: &str, name: Option<&str>) -> Option<String> {
    let bytes = get(key, name, RRF_RT_REG_SZ)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    Some(from_buffer(&units))
}

/// A DWORD; `None` when missing or of another type.
pub fn get_dword(key: &str, name: &str) -> Option<u32> {
    let bytes = get(key, Some(name), RRF_RT_REG_DWORD)?;
    Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
}

pub fn set_dword(key: &str, name: &str, value: u32) -> windows::core::Result<()> {
    set(key, Some(name), REG_DWORD, &value.to_le_bytes())
}

pub fn delete_value(key: &str, name: &str) {
    let (k, n) = (W::new(Some(key)), W::new(Some(name)));
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, k.ptr(), n.ptr());
    }
}

/// The key, its values and everything below it.
pub fn delete_tree(key: &str) {
    let k = W::new(Some(key));
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, k.ptr());
        let _ = RegDeleteKeyW(HKEY_CURRENT_USER, k.ptr());
    }
}

pub fn exists(key: &str) -> bool {
    let k = W::new(Some(key));
    let mut h = HKEY::default();
    let found = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, k.ptr(), None, KEY_READ, &mut h) }
        == ERROR_SUCCESS;
    if found {
        unsafe {
            let _ = RegCloseKey(h);
        }
    }
    found
}

/// Deletes the key only if it holds neither values nor subkeys.
pub fn delete_if_empty(key: &str) {
    let k = W::new(Some(key));
    let mut h = HKEY::default();
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, k.ptr(), None, KEY_READ, &mut h) != ERROR_SUCCESS {
            return;
        }
        let (mut subkeys, mut values) = (0u32, 0u32);
        let e = RegQueryInfoKeyW(
            h,
            None,
            None,
            None,
            Some(&mut subkeys),
            None,
            None,
            Some(&mut values),
            None,
            None,
            None,
            None,
        );
        let _ = RegCloseKey(h);
        if e == ERROR_SUCCESS && subkeys == 0 && values == 0 {
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, k.ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_and_trees_go() {
        let key = format!(r"Software\FolderTint-Test-{}-reg", std::process::id());
        let sub = format!(r"{key}\a\b");
        set_string(&sub, None, "Índigo «x»").unwrap();
        set_string(&sub, Some("n"), "").unwrap();

        set_dword(&sub, "d", 0x20).unwrap();
        assert_eq!(get_string(&sub, None).as_deref(), Some("Índigo «x»"));
        assert_eq!(get_string(&sub, Some("n")).as_deref(), Some(""));
        assert_eq!(get_dword(&sub, "d"), Some(0x20));
        assert_eq!(get_dword(&sub, "n"), None, "a string is not a DWORD");
        assert_eq!(get_string(&sub, Some("missing")), None);
        delete_value(&sub, "d");
        assert_eq!(get_dword(&sub, "d"), None);

        delete_if_empty(&key);
        assert!(exists(&key), "a key with subkeys stays");
        delete_tree(&sub);
        assert!(!exists(&sub));
        delete_if_empty(&format!(r"{key}\a"));
        delete_if_empty(&key);
        assert!(!exists(&key));
    }
}
