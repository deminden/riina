//! Desktop drop data and routing contracts shared by native targets.
use slint::{ComponentHandle, LogicalPosition, Weak, Window};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use super::macos::file_drop as native;
#[cfg(target_os = "linux")]
use super::wayland_drop as native;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "windows")]
use super::windows::file_drop as native;

pub(crate) struct Registration {
    native: native::Registration,
    #[cfg(target_os = "linux")]
    x11: x11::Registration,
}

impl Registration {
    pub(crate) fn attach(&self, window: &Window) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.native.attach(window);
        #[cfg(target_os = "linux")]
        {
            let _ = &self.native;
            self.x11.attach(window);
        }
    }

    pub(crate) fn attach_owner(&self, owner: Option<Weak<crate::NativeVideoHost>>) {
        #[cfg(target_os = "windows")]
        if let Some(owner) = owner.and_then(|owner| owner.upgrade()) {
            self.native.attach_owner(owner.window());
        }
        #[cfg(not(target_os = "windows"))]
        let _ = owner;
    }
}

pub(crate) fn register<T: ComponentHandle + 'static>(
    window: &T,
    hover: impl Fn(Hover) + 'static,
    capture: impl Fn(LogicalPosition) -> Option<OpenDrop> + 'static,
) -> Registration {
    #[cfg(target_os = "linux")]
    {
        let hover = std::rc::Rc::new(hover);
        let capture = std::rc::Rc::new(capture);
        let native_hover = hover.clone();
        let native_capture = capture.clone();
        Registration {
            native: native::register(
                window,
                move |event| native_hover(event),
                move |position| native_capture(position),
            ),
            x11: x11::Registration::new(
                window,
                move |event| hover(event),
                move |position| capture(position),
            ),
        }
    }
    #[cfg(not(target_os = "linux"))]
    Registration {
        native: native::register(window, hover, capture),
    }
}

pub(crate) enum Hover {
    Entered {
        paths: Vec<PathBuf>,
        position: LogicalPosition,
        known_playable: bool,
    },
    Moved(LogicalPosition),
    Left,
}

impl Hover {
    pub(super) fn files(paths: Vec<PathBuf>, position: LogicalPosition) -> Self {
        Self::Entered {
            paths,
            position,
            known_playable: false,
        }
    }
}

pub(crate) type OpenDrop = Box<dyn FnOnce(Vec<OsString>)>;

pub(super) fn parse_references(text: &str, allow_plain_paths: bool) -> Vec<OsString> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.contains('\0'))
        .filter_map(|line| parse_reference(line, allow_plain_paths))
        .collect()
}

fn parse_reference(line: &str, allow_plain_paths: bool) -> Option<OsString> {
    if allow_plain_paths && Path::new(line).is_absolute() {
        return Some(line.into());
    }
    let uri = url::Url::parse(line).ok()?;
    if uri.scheme() == "file" {
        return uri.to_file_path().ok().map(|path| path.into_os_string());
    }
    riina_core::media_reference::validate_network_url(line)
        .ok()
        .map(OsString::from)
}
