//! Decode the advertised OLE transfer formats without taking clipboard ownership.
use crate::platform::desktop_drop::parse_references;
use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr::NonNull, slice};
use windows::{
    Win32::{
        Foundation::HGLOBAL,
        Globalization::{CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar},
        System::{
            Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL},
            DataExchange::RegisterClipboardFormatW,
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
            Ole::{CF_HDROP, CF_UNICODETEXT, ReleaseStgMedium},
        },
        UI::Shell::{CFSTR_INETURLA, CFSTR_INETURLW, DragQueryFileW, HDROP},
    },
    core::w,
};

const MAX_MEDIUM: usize = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Files,
    WideLink,
    AnsiLink,
    UriList,
    Text,
}

#[derive(Clone, Copy)]
pub(super) struct Format {
    pub(super) kind: Kind,
    id: u16,
}

thread_local! {
    static FORMATS: Vec<Format> = {
        // SAFETY: the clipboard format names are static and NUL terminated.
        let (wide_url, ansi_url, uris) = unsafe {
            (
                RegisterClipboardFormatW(CFSTR_INETURLW),
                RegisterClipboardFormatW(CFSTR_INETURLA),
                RegisterClipboardFormatW(w!("text/uri-list")),
            )
        };
        [
            (Kind::Files, u32::from(CF_HDROP.0)),
            (Kind::WideLink, wide_url),
            (Kind::AnsiLink, ansi_url),
            (Kind::UriList, uris),
            (Kind::Text, u32::from(CF_UNICODETEXT.0)),
        ]
        .into_iter()
        .filter_map(|(kind, id)| Some(Format { kind, id: u16::try_from(id).ok()? }))
        .filter(|format| format.id != 0)
        .collect()
    };
}

pub(super) fn available(source: &IDataObject) -> Vec<Format> {
    FORMATS.with(|formats| {
        formats
            .iter()
            .copied()
            .filter(|format| {
                // SAFETY: QueryGetData borrows an initialized FORMATETC for this call.
                unsafe { source.QueryGetData(&format.descriptor()).is_ok() }
            })
            .collect()
    })
}

impl Format {
    fn descriptor(self) -> FORMATETC {
        FORMATETC {
            cfFormat: self.id,
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
            ..Default::default()
        }
    }

    pub(super) fn read(self, source: &IDataObject) -> Vec<OsString> {
        // SAFETY: GetData borrows the initialized descriptor and returns an owned medium.
        let Ok(medium) = (unsafe { source.GetData(&self.descriptor()) }) else {
            return Vec::new();
        };
        let medium = Medium(medium);
        let Some(memory) = medium.lock() else {
            return Vec::new();
        };
        let bytes = memory.bytes();
        if self.kind == Kind::Files {
            return file_paths(memory.handle, bytes);
        }
        decode_text(self.kind, bytes)
            .map(|text| parse_references(&text, self.kind == Kind::Text))
            .unwrap_or_default()
    }
}

struct Medium(STGMEDIUM);

impl Medium {
    fn lock(&self) -> Option<LockedMemory<'_>> {
        if self.0.tymed != TYMED_HGLOBAL.0 as u32 {
            return None;
        }
        // SAFETY: the negotiated medium is TYMED_HGLOBAL. It remains owned by self.
        let handle = unsafe { self.0.u.hGlobal };
        // SAFETY: GlobalSize and GlobalLock take the live HGLOBAL from this medium.
        let size = unsafe { GlobalSize(handle) };
        if size == 0 || size > MAX_MEDIUM {
            return None;
        }
        // SAFETY: the validated size comes from this live HGLOBAL.
        let pointer = NonNull::new(unsafe { GlobalLock(handle) }.cast::<u8>())?;
        Some(LockedMemory {
            handle,
            pointer,
            size,
            _medium: self,
        })
    }
}

impl Drop for Medium {
    fn drop(&mut self) {
        // SAFETY: GetData transferred release responsibility to this guard.
        unsafe { ReleaseStgMedium(&mut self.0) };
    }
}

struct LockedMemory<'a> {
    handle: HGLOBAL,
    pointer: NonNull<u8>,
    size: usize,
    _medium: &'a Medium,
}

impl LockedMemory<'_> {
    fn bytes(&self) -> &[u8] {
        // SAFETY: GlobalLock pins at least GlobalSize bytes until this guard unlocks.
        unsafe { slice::from_raw_parts(self.pointer.as_ptr(), self.size) }
    }
}

impl Drop for LockedMemory<'_> {
    fn drop(&mut self) {
        // SAFETY: this guard represents one successful GlobalLock.
        let _ = unsafe { GlobalUnlock(self.handle) };
    }
}

fn decode_text(kind: Kind, bytes: &[u8]) -> Option<String> {
    match kind {
        Kind::WideLink | Kind::Text => {
            let characters = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .take_while(|unit| *unit != 0)
                .collect::<Vec<_>>();
            String::from_utf16(&characters).ok()
        }
        Kind::AnsiLink => decode_ansi(before_nul(bytes)),
        Kind::UriList => String::from_utf8(before_nul(bytes).to_vec()).ok(),
        Kind::Files => None,
    }
}

fn before_nul(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())]
}

fn decode_ansi(bytes: &[u8]) -> Option<String> {
    let flags = MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0);
    // SAFETY: both calls borrow valid slices. None queries the required length.
    let length = unsafe { MultiByteToWideChar(CP_ACP, flags, bytes, None) };
    let mut wide = vec![0; usize::try_from(length).ok()?];
    // SAFETY: wide has the requested capacity and is writable for the call.
    let written = unsafe { MultiByteToWideChar(CP_ACP, flags, bytes, Some(&mut wide)) };
    (written > 0 && written == length).then(|| String::from_utf16_lossy(&wide))
}

fn file_paths(handle: HGLOBAL, bytes: &[u8]) -> Vec<OsString> {
    if !valid_dropfiles(bytes) {
        return Vec::new();
    }
    let drop = HDROP(handle.0);
    // SAFETY: the medium holds a DROPFILES list with a terminator inside its allocation.
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    (0..count)
        .filter_map(|index| {
            // SAFETY: the same list remains locked. The buffer includes the final NUL.
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            let mut path = vec![0; length + 1];
            // SAFETY: the buffer matches the length reported for this index.
            let written = unsafe { DragQueryFileW(drop, index, Some(&mut path)) } as usize;
            (written > 0 && written == length).then(|| OsString::from_wide(&path[..length]))
        })
        .collect()
}

fn valid_dropfiles(bytes: &[u8]) -> bool {
    // DROPFILES contains five 32-bit fields. pFiles is relative to its beginning.
    if bytes.len() < 20 {
        return false;
    }
    let offset = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let Some(files) = bytes.get(offset..) else {
        return false;
    };
    if offset < 20 {
        return false;
    }
    if bytes[16..20] == [0; 4] {
        files.windows(2).any(|pair| pair == [0; 2])
    } else {
        offset.is_multiple_of(2)
            && files
                .as_chunks::<2>()
                .0
                .windows(2)
                .any(|pair| pair == [[0; 2]; 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::{
        System::{
            Com::STGMEDIUM_0,
            Memory::{GHND, GlobalAlloc},
        },
        UI::Shell::SHCreateDataObject,
    };

    fn add_data(source: &IDataObject, kind: Kind, bytes: &[u8]) -> windows::core::Result<()> {
        let format =
            FORMATS.with(|formats| formats.iter().find(|format| format.kind == kind).copied());
        let Some(format) = format else {
            return Err(windows::Win32::Foundation::E_FAIL.into());
        };
        // SAFETY: allocate a moveable transfer block for this test's bytes.
        let handle = unsafe { GlobalAlloc(GHND, bytes.len()) }?;
        let medium = Medium(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: handle },
            ..Default::default()
        });
        {
            let Some(memory) = medium.lock() else {
                return Err(windows::Win32::Foundation::E_FAIL.into());
            };
            // SAFETY: the locked block has room for the supplied bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), memory.pointer.as_ptr(), bytes.len())
            };
        }
        // SAFETY: on success the shell object takes responsibility for releasing the medium.
        unsafe { source.SetData(&format.descriptor(), &medium.0, true) }?;
        std::mem::forget(medium);
        Ok(())
    }

    fn wide_bytes(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn shell_data_objects_deliver_unicode_files_and_browser_links() -> windows::core::Result<()> {
        let _apartment = super::super::Apartment::new()?;
        // SAFETY: request an empty Shell data object for locally supplied formats.
        let source: IDataObject = unsafe { SHCreateDataObject(None, None, None::<&IDataObject>) }?;
        let mut files = vec![0; 20];
        files[0] = 20;
        files[16] = 1;
        files.extend(wide_bytes(
            "C:\\test\\clip α.mkv\0C:\\test\\captions.srt\0\0",
        ));
        add_data(&source, Kind::Files, &files)?;
        add_data(
            &source,
            Kind::WideLink,
            &wide_bytes("https://example.test/video\0"),
        )?;
        add_data(
            &source,
            Kind::Text,
            &wide_bytes("https://example.test/fallback\0"),
        )?;
        let formats = available(&source);
        let decoded = formats
            .iter()
            .map(|format| format.read(&source))
            .filter(|paths| !paths.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(
            decoded,
            [
                vec![
                    OsString::from("C:\\test\\clip α.mkv"),
                    OsString::from("C:\\test\\captions.srt")
                ],
                vec![OsString::from("https://example.test/video")],
                vec![OsString::from("https://example.test/fallback")],
            ]
        );
        assert!(!valid_dropfiles(&files[..files.len() - 2]));
        Ok(())
    }
}
