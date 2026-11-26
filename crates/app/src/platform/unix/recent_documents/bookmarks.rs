//! Own the GLib bookmark document and its transferred buffers.
use gtk::glib::{self, ffi, translate::from_glib_full};
use std::{
    ffi::CStr,
    io,
    ptr::{self, NonNull},
};

pub(super) struct Bookmarks(NonNull<ffi::GBookmarkFile>);

impl Bookmarks {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: the constructor returns a new owned bookmark document.
        let raw = unsafe { ffi::g_bookmark_file_new() };
        NonNull::new(raw)
            .map(Self)
            .ok_or_else(|| io::Error::other("Could not create the desktop bookmark document"))
    }

    pub(super) fn load(&mut self, data: &[u8]) -> io::Result<()> {
        let mut error = ptr::null_mut();
        // SAFETY: the byte range stays live and GLib initializes the error on failure.
        let loaded = unsafe {
            ffi::g_bookmark_file_load_from_data(
                self.0.as_ptr(),
                data.as_ptr(),
                data.len(),
                &mut error,
            )
        };
        if loaded == ffi::GFALSE {
            Err(bookmark_error(error))
        } else {
            Ok(())
        }
    }

    pub(super) fn to_bytes(&self) -> io::Result<glib::Slice<u8>> {
        let mut length = 0;
        let mut error = ptr::null_mut();
        // SAFETY: the out parameters are writable and the document remains alive.
        let data =
            unsafe { ffi::g_bookmark_file_to_data(self.0.as_ptr(), &mut length, &mut error) };
        if data.is_null() {
            return Err(bookmark_error(error));
        }
        // SAFETY: GLib transfers the returned byte buffer and its length.
        Ok(unsafe { glib::Slice::from_glib_full_num(data, length) })
    }

    pub(super) fn register(&mut self, uri: &CStr, mime: &CStr, app: &CStr, command: &CStr) {
        let bookmark = self.0.as_ptr();
        // SAFETY: all inputs are live NUL-terminated strings copied by the document.
        unsafe {
            if ffi::g_bookmark_file_has_item(bookmark, uri.as_ptr()) == ffi::GFALSE {
                ffi::g_bookmark_file_set_mime_type(bookmark, uri.as_ptr(), mime.as_ptr());
            }
            ffi::g_bookmark_file_add_application(
                bookmark,
                uri.as_ptr(),
                app.as_ptr(),
                command.as_ptr(),
            );
        }
    }

    pub(super) fn remove_application(&mut self, app: &CStr) -> bool {
        let bookmark = self.0.as_ptr();
        // SAFETY: GLib returns an independent, owned vector of copied URIs.
        let uris = unsafe {
            glib::StrV::from_glib_full(ffi::g_bookmark_file_get_uris(bookmark, ptr::null_mut()))
        };
        let mut changed = false;
        for uri in &uris {
            let uri = uri.as_ptr();
            // SAFETY: URI and app strings live through the call. False means this app
            // has no registration to remove for the known URI.
            let removed = unsafe {
                ffi::g_bookmark_file_remove_application(
                    bookmark,
                    uri,
                    app.as_ptr(),
                    ptr::null_mut(),
                )
            };
            if removed == ffi::GFALSE {
                continue;
            }
            changed = true;
            // SAFETY: the URI is still owned by the vector. GLib transfers the application names.
            let applications = unsafe {
                glib::StrV::from_glib_full(ffi::g_bookmark_file_get_applications(
                    bookmark,
                    uri,
                    ptr::null_mut(),
                    ptr::null_mut(),
                ))
            };
            if applications.is_empty() {
                // SAFETY: only the bookmark whose final registration we just removed is deleted.
                unsafe {
                    ffi::g_bookmark_file_remove_item(bookmark, uri, ptr::null_mut());
                }
            }
        }
        changed
    }
}

impl Drop for Bookmarks {
    fn drop(&mut self) {
        // SAFETY: this handle uniquely owns the live GLib document.
        unsafe { ffi::g_bookmark_file_free(self.0.as_ptr()) }
    }
}

fn bookmark_error(error: *mut ffi::GError) -> io::Error {
    if error.is_null() {
        return io::Error::other("Could not process the desktop recent-file list");
    }
    // SAFETY: the failed GLib operation transferred its allocated error.
    let error: glib::Error = unsafe { from_glib_full(error) };
    io::Error::other(error)
}
