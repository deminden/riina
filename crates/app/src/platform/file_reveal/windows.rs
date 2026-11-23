use ::windows::{
    Win32::{
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        UI::Shell::{
            Common::ITEMIDLIST, ILFindLastID, SHOpenFolderAndSelectItems, SHParseDisplayName,
        },
    },
    core::{HRESULT, PCWSTR},
};
use std::{
    collections::BTreeMap,
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) fn reveal(paths: &[PathBuf]) -> io::Result<()> {
    // SAFETY: COM is initialized on the calling thread and balanced by the guard.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if !initialized.is_ok() && initialized != HRESULT(0x80010106_u32 as i32) {
        return Err(io::Error::other(initialized.to_string()));
    }
    let _apartment = Apartment {
        owned: initialized.is_ok(),
    };
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for path in paths {
        let folder = path
            .parent()
            .ok_or_else(|| io::Error::other("The file has no parent folder"))?;
        groups.entry(folder).or_default().push(path);
    }
    for (folder, files) in groups {
        let parent = ItemIdList::parse(folder)?;
        let items = files
            .into_iter()
            .map(|path| ItemIdList::parse(path))
            .collect::<io::Result<Vec<_>>>()?;
        let children: Vec<*const ITEMIDLIST> = items
            .iter()
            .map(|item| {
                // SAFETY: the relative child points into the retained absolute ID list.
                unsafe { ILFindLastID(item.0).cast_const() }
            })
            .collect();
        // SAFETY: both the parent and all relative child IDs remain live for this call.
        unsafe { SHOpenFolderAndSelectItems(parent.0, Some(&children), 0) }
            .map_err(io::Error::other)?;
    }
    Ok(())
}

struct Apartment {
    owned: bool,
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: the guard is dropped on the same thread that acquired COM.
            unsafe { CoUninitialize() };
        }
    }
}

struct ItemIdList(*mut ITEMIDLIST);

impl ItemIdList {
    fn parse(path: &Path) -> io::Result<Self> {
        let mut text = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if text.contains(&0) {
            return Err(io::Error::other("The file path contains a NUL character"));
        }
        text.push(0);
        let mut item = Self(std::ptr::null_mut());
        // SAFETY: the terminated UTF-16 string and writable output pointer are live.
        unsafe { SHParseDisplayName(PCWSTR(text.as_ptr()), None, &mut item.0, 0, None) }
            .map_err(io::Error::other)?;
        if item.0.is_null() {
            return Err(io::Error::other("Explorer did not identify the file"));
        }
        Ok(item)
    }
}

impl Drop for ItemIdList {
    fn drop(&mut self) {
        // SAFETY: Shell allocated this ID list with COM's task allocator.
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}
