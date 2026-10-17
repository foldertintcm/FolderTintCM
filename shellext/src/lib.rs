//! FolderTintCM's Explorer menu for the Microsoft Store package: an
//! in-process COM server whose `IExplorerCommand` the package declares for
//! folders and folder backgrounds (`desktop4:FileExplorerContextMenus`).
//! On Windows 11 it shows in the first menu, without "Show more options".
//!
//! It only builds the submenu (`menu.rs`) and starts `foldertintcm.exe`
//! with the chosen action; the program does the work, as for the MSI.
//!
//! This code runs inside Explorer, so it never panics on purpose: no
//! indexing or `unwrap`, every failure becomes an HRESULT.

#![allow(non_snake_case)]

#[allow(dead_code)]
#[path = "../../src/package.rs"]
mod package;
#[allow(dead_code)]
#[path = "../../src/palette.rs"]
mod palette;

mod menu;

use std::cell::Cell;
use std::ffi::c_void;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, CloseHandle, E_NOTIMPL, E_POINTER, HMODULE,
    S_FALSE, S_OK,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Threading::{
    CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Shell::{
    ECF_HASSUBCOMMANDS, ECF_SEPARATORBEFORE, ECS_ENABLED, ECS_HIDDEN, IEnumExplorerCommand,
    IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray, SHStrDupW,
    SIGDN_FILESYSPATH,
};
use windows::core::{
    BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref, Result, implement,
};

use menu::Entry;

/// The class the package manifest names (`com:Class Id`, `desktop5:Verb Clsid`).
pub const CLSID: GUID = GUID::from_u128(0x73bece91_1b44_47c5_a536_a0552e5387c6);

/// This DLL's folder: the package's folder, where the program and the
/// palette's icons are.
fn package_dir() -> Option<PathBuf> {
    let mut module = HMODULE::default();
    let anchor = package_dir as *const c_void;
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(anchor.cast()),
            &mut module,
        )
        .ok()?;
    }
    let mut buf = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(Some(module), &mut buf) } as usize;
    let path = PathBuf::from(String::from_utf16_lossy(buf.get(..len)?));
    path.parent().map(PathBuf::from)
}

/// The program's data folder, as it computes it inside the package.
fn data_dir(package: &std::path::Path) -> Option<PathBuf> {
    let (_, family) = package::containing(&package.join("foldertintcm.exe"))?;
    package::local_cache(&family).map(|d| d.join("FolderTintCM"))
}

fn spanish() -> bool {
    (unsafe { GetUserDefaultUILanguage() } & 0x3FF) == 0x0A
}

fn dup(text: &str) -> Result<PWSTR> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    unsafe { SHStrDupW(PCWSTR(wide.as_ptr())) }
}

/// The folders an entry acts on: the selection, or the open folder for a
/// right click on its background.
fn folders(items: &IShellItemArray) -> Result<Vec<String>> {
    let count = unsafe { items.GetCount()? };
    let mut out = Vec::new();
    for i in 0..count {
        let item = unsafe { items.GetItemAt(i)? };
        let Ok(name) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
            // Not a file-system folder (a library, "This PC"): skipped.
            continue;
        };
        let text = unsafe { name.to_string() }.unwrap_or_default();
        unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
        if !text.is_empty() {
            out.push(text);
        }
    }
    Ok(out)
}

/// Starts `command` and lets it run on its own.
fn spawn(command: &str) -> Result<()> {
    let mut line: Vec<u16> = command.encode_utf16().collect();
    line.push(0);
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )?;
        let _ = CloseHandle(info.hProcess);
        let _ = CloseHandle(info.hThread);
    }
    Ok(())
}

/// "FolderTintCM ›": the parent entry.
#[implement(IExplorerCommand)]
struct Root;

impl IExplorerCommand_Impl for Root_Impl {
    fn GetTitle(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        dup("FolderTintCM")
    }

    fn GetIcon(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        let dir = package_dir().ok_or(windows::core::Error::from(E_NOTIMPL))?;
        dup(&format!("{},0", dir.join("foldertintcm.exe").display()))
    }

    fn GetToolTip(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLSID)
    }

    fn GetState(&self, _: Ref<IShellItemArray>, _: BOOL) -> Result<u32> {
        Ok(ECS_ENABLED.0 as u32)
    }

    fn Invoke(&self, _: Ref<IShellItemArray>, _: Ref<IBindCtx>) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_HASSUBCOMMANDS.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        let package = package_dir().ok_or(windows::core::Error::from(E_NOTIMPL))?;
        let data = data_dir(&package);
        let recent = data
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("recent.txt")).ok())
            .map(|text| menu::parse_recent(&text))
            .unwrap_or_default();
        let exe = package.join("foldertintcm.exe");
        let items: Vec<IExplorerCommand> =
            menu::entries(spanish(), &package, data.as_deref(), &recent)
                .into_iter()
                .map(|entry| {
                    Sub {
                        entry,
                        exe: exe.clone(),
                    }
                    .into()
                })
                .collect();
        Ok(Enum {
            items,
            next: Cell::new(0),
        }
        .into())
    }
}

/// One colour, "Custom…" or "Remove colour".
#[implement(IExplorerCommand)]
struct Sub {
    entry: Entry,
    exe: PathBuf,
}

impl IExplorerCommand_Impl for Sub_Impl {
    fn GetTitle(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        dup(&self.entry.title)
    }

    fn GetIcon(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        if self.entry.icon.is_empty() {
            return Err(E_NOTIMPL.into());
        }
        dup(&self.entry.icon)
    }

    fn GetToolTip(&self, _: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Err(E_NOTIMPL.into())
    }

    fn GetState(&self, items: Ref<IShellItemArray>, _: BOOL) -> Result<u32> {
        // On a drive's root (`D:\`, or the background of an open drive) a
        // colour would not show: only "Remove colour" stays.
        let roots_only = self.entry.action != menu::Action::Clear
            && items
                .ok()
                .ok()
                .and_then(|items| folders(items).ok())
                .is_some_and(|f| !f.is_empty() && !f.iter().any(|f| menu::colourable(f)));
        Ok(if roots_only { ECS_HIDDEN } else { ECS_ENABLED }.0 as u32)
    }

    fn Invoke(&self, items: Ref<IShellItemArray>, _: Ref<IBindCtx>) -> Result<()> {
        let items = items.ok()?;
        // One process per folder, as Explorer does for the MSI's verbs;
        // "Custom…" opens a single picker for all of them.
        for folder in folders(items)? {
            let _ = spawn(&menu::command_line(&self.exe, &self.entry.action, &folder));
        }
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(if self.entry.separator_before {
            ECF_SEPARATORBEFORE.0 as u32
        } else {
            0
        })
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IEnumExplorerCommand)]
struct Enum {
    items: Vec<IExplorerCommand>,
    next: Cell<usize>,
}

impl IEnumExplorerCommand_Impl for Enum_Impl {
    fn Next(&self, celt: u32, out: *mut Option<IExplorerCommand>, fetched: *mut u32) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let mut n = 0u32;
        while n < celt {
            let Some(item) = self.items.get(self.next.get()) else {
                break;
            };
            unsafe { out.add(n as usize).write(Some(item.clone())) };
            self.next.set(self.next.get() + 1);
            n += 1;
        }
        if !fetched.is_null() {
            unsafe { *fetched = n };
        }
        if n == celt { S_OK } else { S_FALSE }
    }

    fn Skip(&self, celt: u32) -> Result<()> {
        self.next.set(
            self.next
                .get()
                .saturating_add(celt as usize)
                .min(self.items.len()),
        );
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        Ok(Enum {
            items: self.items.clone(),
            next: Cell::new(self.next.get()),
        }
        .into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if object.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = Root.into();
        unsafe { command.query(riid, object) }.ok()
    }

    fn LockServer(&self, _: BOOL) -> Result<()> {
        Ok(())
    }
}

/// COM's entry point: the factory of `CLSID`, the only class here.
///
/// # Safety
/// COM passes valid pointers to a GUID, an IID and an out-pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if object.is_null() || clsid.is_null() || iid.is_null() {
        return E_POINTER;
    }
    unsafe { *object = std::ptr::null_mut() };
    if unsafe { *clsid } != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(iid, object) }
}

/// Kept loaded for Explorer's lifetime: a menu extension is small and
/// unloading one with an object still referenced would crash Explorer.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}
