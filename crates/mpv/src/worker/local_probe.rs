//! Own a silent, local-file-only decoder independent of the player.
use super::handle::Handle;
use crate::ffi::Api;
use std::{
    ffi::{CStr, CString},
    path::Path,
};

pub(super) struct LocalProbe {
    pub(super) handle: Handle,
    #[cfg(windows)]
    _apartment: crate::windows_apartment::Apartment,
}

impl LocalProbe {
    pub(super) fn open(library: &Path, file: &Path, options: &[(&CStr, &CStr)]) -> Option<Self> {
        if !file.is_absolute() || !file.is_file() {
            return None;
        }
        #[cfg(windows)]
        let apartment = crate::windows_apartment::Apartment::initialize().ok()?;
        let handle = Handle::new(Api::load(library).ok()?).ok()?;
        for (key, value) in [
            (c"config", c"no"),
            (c"load-scripts", c"no"),
            (c"autoload-files", c"no"),
            (c"access-references", c"no"),
            (c"demuxer-lavf-o", c"protocol_whitelist=file"),
            (c"ao", c"null"),
            (c"vo", c"null"),
            (c"pause", c"yes"),
        ]
        .into_iter()
        .chain(options.iter().copied())
        {
            handle.set_option(key, value).ok()?;
        }
        // Some builds omit the optional ytdl hook and its configuration option.
        let _ = handle.set_option(c"ytdl", c"no");
        handle.initialize().ok()?;
        handle
            .command(1, &[c"loadfile".into(), CString::new(file.to_str()?).ok()?])
            .ok()?;
        Some(Self {
            handle,
            #[cfg(windows)]
            _apartment: apartment,
        })
    }
}
