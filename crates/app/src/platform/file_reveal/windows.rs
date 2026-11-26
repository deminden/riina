use ::windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        UI::Shell::{
            Common::ITEMIDLIST, ILFindLastID, SHOpenFolderAndSelectItems, SHParseDisplayName,
        },
    },
    core::PCWSTR,
};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::OsString,
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Component, Path, PathBuf, Prefix},
};

pub(super) fn reveal(paths: &[PathBuf]) -> io::Result<()> {
    let _apartment = Apartment::initialize()?;
    if let [path] = paths {
        let item = ItemIdList::parse(path)?;
        // SAFETY: with no child array Explorer opens the parent of this absolute item ID.
        return unsafe { SHOpenFolderAndSelectItems(item.0, None, 0) }.map_err(io::Error::other);
    }
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

impl Apartment {
    fn initialize() -> io::Result<Self> {
        // SAFETY: COM is initialized on the calling thread and balanced by the guard.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if !result.is_ok() && result != RPC_E_CHANGED_MODE {
            return Err(io::Error::other(result.to_string()));
        }
        Ok(Self {
            owned: result.is_ok(),
        })
    }
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
        let path = shell_spelling(path)?;
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

fn shell_spelling(path: &Path) -> io::Result<Cow<'_, Path>> {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Ok(Cow::Borrowed(path));
    };
    let (skip, leading): (_, &[u16]) = match prefix.kind() {
        Prefix::VerbatimDisk(_) => (4, &[]),
        Prefix::VerbatimUNC(..) => (8, &[u16::from(b'\\'); 2]),
        _ => return Ok(Cow::Borrowed(path)),
    };
    let wide: Vec<_> = leading
        .iter()
        .copied()
        .chain(path.as_os_str().encode_wide().skip(skip))
        .collect();
    let ordinary = PathBuf::from(OsString::from_wide(&wide));
    // Explorer rejects canonical Win32 prefixes. Strip one only if normal
    // Win32 resolution spells the same canonical path, including its final entry.
    if ordinary.canonicalize()?.as_os_str() != path.as_os_str() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Explorer cannot address this filesystem path",
        ));
    }
    Ok(Cow::Owned(ordinary))
}

impl Drop for ItemIdList {
    fn drop(&mut self) {
        // SAFETY: Shell allocated this ID list with COM's task allocator.
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Shell::{GPFIDL_DEFAULT, SHGetPathFromIDListEx};

    #[test]
    fn explorer_identifies_canonical_unicode_files_and_directories() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Archive ü");
        std::fs::create_dir(&folder).unwrap();
        let file = folder.join("Episode ü %, #.mkv");
        std::fs::write(&file, []).unwrap();
        let _apartment = Apartment::initialize().unwrap();
        for entry in [folder, file.clone()] {
            let canonical = entry.canonicalize().unwrap();
            let item = ItemIdList::parse(&canonical).unwrap();
            let mut wide = [0; 1024];
            // SAFETY: the shell ID is live and the slice is writable for the call.
            assert!(unsafe { SHGetPathFromIDListEx(item.0, &mut wide, GPFIDL_DEFAULT) }.as_bool());
            let len = wide.iter().position(|&unit| unit == 0).unwrap();
            let selected = PathBuf::from(OsString::from_wide(&wide[..len]));
            assert_eq!(selected.canonicalize().unwrap(), canonical);
        }
        let different = file.canonicalize().unwrap().with_extension("mkv.");
        assert!(shell_spelling(&different).is_err());
    }
}
