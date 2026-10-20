//! Native services are isolated here; core state and the shared UI own behavior.
use crate::{NativeVideoHost, PlayerWindow};
#[cfg(target_os = "windows")]
use slint::winit_030::winit::window::Theme as WinitTheme;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
#[cfg(target_os = "windows")]
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
pub(crate) mod clipboard;
pub(crate) mod folder_dialog;
pub(crate) mod language;
mod plugin_resources;
pub(crate) mod settings_file;

#[cfg(target_os = "windows")]
type MaterialPolicyMonitor = Box<dyn FnMut(bool) -> bool>;
#[cfg(target_os = "windows")]
type RegisteredMaterialPolicyMonitor = (isize, WindowsBackdropKind, MaterialPolicyMonitor);

#[cfg(target_os = "windows")]
thread_local! {
    static MATERIAL_POLICY_MONITORS: RefCell<Vec<RegisteredMaterialPolicyMonitor>> = const { RefCell::new(Vec::new()) };
    static MATERIAL_POLICY_TIMER: RefCell<Option<slint::Timer>> = const { RefCell::new(None) };
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum WindowsBackdropKind {
    Persistent,
    Transient,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct WindowsBackdrop {
    title: &'static str,
    kind: WindowsBackdropKind,
    initially_ready: bool,
}

#[cfg(target_os = "windows")]
fn set_windows_backdrop(
    kind: WindowsBackdropKind,
    window: &slint::winit_030::winit::window::Window,
    enabled: bool,
) -> Result<(), String> {
    if !enabled {
        return windows::materials::clear_system_backdrop(window);
    }
    match kind {
        WindowsBackdropKind::Persistent => windows::materials::apply_main_window_mica(window),
        WindowsBackdropKind::Transient => {
            windows::materials::apply_transient_window_acrylic(window)
        }
    }
}

#[cfg(target_os = "windows")]
fn backdrop_policy_enabled(kind: WindowsBackdropKind) -> bool {
    match kind {
        WindowsBackdropKind::Persistent => windows::materials::native_mica_enabled(),
        WindowsBackdropKind::Transient => windows::materials::native_materials_enabled(),
    }
}

#[cfg(target_os = "windows")]
fn register_material_policy_monitor(
    identity: isize,
    kind: WindowsBackdropKind,
    monitor: MaterialPolicyMonitor,
) {
    MATERIAL_POLICY_MONITORS.with(|monitors| {
        let mut monitors = monitors.borrow_mut();
        monitors.retain(|(registered_identity, _, _)| *registered_identity != identity);
        monitors.push((identity, kind, monitor));
    });
    MATERIAL_POLICY_TIMER.with(|timer_slot| {
        let mut timer_slot = timer_slot.borrow_mut();
        let timer = timer_slot.get_or_insert_with(slint::Timer::default);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            || {
                MATERIAL_POLICY_MONITORS.with(|monitors| {
                    monitors
                        .borrow_mut()
                        .retain_mut(|(_, kind, monitor)| monitor(backdrop_policy_enabled(*kind)));
                    if monitors.borrow().is_empty() {
                        MATERIAL_POLICY_TIMER.with(|timer| {
                            if let Some(timer) = timer.borrow().as_ref() {
                                timer.stop();
                            }
                        });
                    }
                });
            },
        );
    });
}

#[cfg(target_os = "windows")]
fn watch_windows_backdrop_policy<W, Get, Set, GetTheme>(
    window: &W,
    identity: isize,
    backdrop: WindowsBackdrop,
    initial_policy: bool,
    get_enabled: Get,
    set_enabled: Set,
    get_theme: GetTheme,
) where
    W: ComponentHandle + 'static,
    Get: Fn(&W) -> bool + 'static,
    Set: Fn(&W, bool) + 'static,
    GetTheme: Fn(&W) -> WinitTheme + 'static,
{
    let weak = window.as_weak();
    let mut last_policy = initial_policy;
    let mut last_theme = get_theme(window);
    let mut backdrop_ready = backdrop.initially_ready;
    let mut retry_delay = Duration::from_millis(500);
    let mut retry_after = Instant::now();
    let mut last_error = None::<String>;
    let monitor = Box::new(move |policy_enabled| {
        let Some(window) = weak.upgrade() else {
            return false;
        };
        let theme = get_theme(&window);
        if theme != last_theme {
            window
                .window()
                .with_winit_window(|native| native.set_theme(Some(theme)));
            last_theme = theme;
        }
        let policy_changed = last_policy != policy_enabled;
        if policy_changed {
            last_policy = policy_enabled;
            backdrop_ready = false;
            retry_delay = Duration::from_millis(500);
            retry_after = Instant::now();
            last_error = None;
        }
        if backdrop_ready && get_enabled(&window) == policy_enabled {
            return true;
        }
        if Instant::now() < retry_after {
            return true;
        }
        let mut update_result = None;
        window.window().with_winit_window(|native| {
            update_result = Some(set_windows_backdrop(backdrop.kind, native, policy_enabled));
        });
        match update_result {
            Some(Ok(())) => {
                set_enabled(&window, policy_enabled);
                backdrop_ready = true;
                retry_delay = Duration::from_millis(500);
                last_error = None;
            }
            Some(Err(error)) => {
                if !policy_enabled {
                    set_enabled(&window, false);
                }
                backdrop_ready = false;
                if last_error.as_deref() != Some(error.as_str()) {
                    eprintln!(
                        "Could not update Windows {} backdrop policy: {error}",
                        backdrop.title
                    );
                    last_error = Some(error);
                }
                retry_after = Instant::now() + retry_delay;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
            }
            None => {
                backdrop_ready = false;
                if !policy_enabled {
                    set_enabled(&window, false);
                }
                retry_after = Instant::now() + retry_delay;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
            }
        }
        true
    });
    register_material_policy_monitor(identity, backdrop.kind, monitor);
}

/// Name the player surfaces accurately on each platform. Windows uses
/// sampled-video compositing in playback; its Mica backdrop is separate from
/// these per-surface controls.
pub(crate) fn surface_material_title() -> &'static str {
    if cfg!(target_os = "macos") {
        "Liquid Glass"
    } else {
        "Translucent Surfaces"
    }
}

/// Reveal the system Mica backdrop under the Windows Welcome surface.
pub(crate) fn enable_welcome_mica(window: &crate::WelcomeWindow) {
    #[cfg(target_os = "windows")]
    enable_windows_backdrop(
        window,
        "Welcome",
        WindowsBackdropKind::Persistent,
        |window| window.get_mica_backdrop_enabled(),
        |window, enabled| window.set_mica_backdrop_enabled(enabled),
        |_| WinitTheme::Dark,
    );
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// Reveal Windows Mica beneath the persistent Preferences window.
pub(crate) fn enable_preferences_mica(window: &crate::PreferencesWindow) {
    #[cfg(target_os = "windows")]
    enable_windows_backdrop(
        window,
        "Preferences",
        WindowsBackdropKind::Persistent,
        |window| window.get_mica_backdrop_enabled(),
        |window, enabled| window.set_mica_backdrop_enabled(enabled),
        |_| WinitTheme::Light,
    );
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// Reveal Windows Desktop Acrylic beneath the transient About window.
pub(crate) fn enable_about_acrylic(window: &crate::AboutWindow) {
    #[cfg(target_os = "windows")]
    enable_transient_acrylic(
        window,
        "About",
        |window| window.get_acrylic_backdrop_enabled(),
        |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
        |window| {
            if window.get_dark_theme() {
                WinitTheme::Dark
            } else {
                WinitTheme::Light
            }
        },
    );
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

pub(crate) fn enable_video_filters_acrylic(window: &crate::VideoFiltersWindow) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Video Filters",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |window| {
                if window.get_dark_theme() {
                    WinitTheme::Dark
                } else {
                    WinitTheme::Light
                }
            },
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

pub(crate) fn enable_history_acrylic(window: &crate::PlaybackHistoryWindow) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Playback History",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |_| WinitTheme::Light,
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

pub(crate) fn enable_log_viewer_acrylic(window: &crate::LogViewerWindow) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Log Viewer",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |_| WinitTheme::Light,
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

pub(crate) fn enable_online_subtitles_acrylic(window: &crate::OnlineSubtitlesWindow) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Online Subtitles",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |_| WinitTheme::Light,
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

pub(crate) fn enable_font_picker_acrylic(window: &crate::FontPicker) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Font Picker",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |window| {
                if window.get_dark_theme() {
                    WinitTheme::Dark
                } else {
                    WinitTheme::Light
                }
            },
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

#[cfg(target_os = "windows")]
pub(crate) fn enable_pip_acrylic(window: &crate::PictureInPictureWindow) {
    #[cfg(target_os = "windows")]
    if !window.get_acrylic_backdrop_enabled() {
        enable_transient_acrylic(
            window,
            "Picture in Picture",
            |window| window.get_acrylic_backdrop_enabled(),
            |window, enabled| window.set_acrylic_backdrop_enabled(enabled),
            |_| WinitTheme::Dark,
        );
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

#[cfg(target_os = "windows")]
fn enable_transient_acrylic<W, Get, Set, GetTheme>(
    window: &W,
    title: &'static str,
    get_enabled: Get,
    set_enabled: Set,
    get_theme: GetTheme,
) where
    W: ComponentHandle + 'static,
    Get: Fn(&W) -> bool + Clone + 'static,
    Set: Fn(&W, bool) + Clone + 'static,
    GetTheme: Fn(&W) -> WinitTheme + Clone + 'static,
{
    enable_windows_backdrop(
        window,
        title,
        WindowsBackdropKind::Transient,
        get_enabled,
        set_enabled,
        get_theme,
    );
}

#[cfg(target_os = "windows")]
fn enable_windows_backdrop<W, Get, Set, GetTheme>(
    window: &W,
    title: &'static str,
    kind: WindowsBackdropKind,
    get_enabled: Get,
    set_enabled: Set,
    get_theme: GetTheme,
) where
    W: ComponentHandle + 'static,
    Get: Fn(&W) -> bool + Clone + 'static,
    Set: Fn(&W, bool) + Clone + 'static,
    GetTheme: Fn(&W) -> WinitTheme + Clone + 'static,
{
    let weak = window.as_weak();
    let get_for_task = get_enabled.clone();
    let set_for_task = set_enabled.clone();
    let theme_for_task = get_theme.clone();
    if let Err(error) = slint::spawn_local(async move {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let policy_enabled = backdrop_policy_enabled(kind);
        let mut backdrop_ready = get_for_task(&window) == policy_enabled;
        let native = match window.window().winit_window().await {
            Ok(native) => native,
            Err(error) => {
                eprintln!("Windows {title} window unavailable; keeping its solid surface: {error}");
                return;
            }
        };
        let identity = match windows::materials::window_identity(&native) {
            Ok(identity) => identity,
            Err(error) => {
                eprintln!("Could not identify the Windows {title} window: {error}");
                return;
            }
        };
        native.set_theme(Some(theme_for_task(&window)));
        if get_for_task(&window) != policy_enabled {
            match set_windows_backdrop(kind, &native, policy_enabled) {
                Ok(()) => {
                    set_for_task(&window, policy_enabled);
                    backdrop_ready = true;
                }
                Err(error) => {
                    if !policy_enabled {
                        set_for_task(&window, false);
                    }
                    backdrop_ready = false;
                    eprintln!(
                        "Windows {title} backdrop unavailable; keeping its solid surface: {error}"
                    );
                }
            }
        }
        watch_windows_backdrop_policy(
            &window,
            identity,
            WindowsBackdrop {
                title,
                kind,
                initially_ready: backdrop_ready,
            },
            policy_enabled,
            get_enabled,
            set_enabled,
            get_theme,
        );
    }) {
        eprintln!("Could not schedule the Windows {title} material: {error}");
    }
}

/// Apply the native minimize action to the window that owns a menu command.
pub(crate) fn minimize_window(window: &slint::Window) {
    window.with_winit_window(|window| window.set_minimized(true));
}

/// Toggle the platform's standard window zoom/maximize state.
pub(crate) fn toggle_zoom_window(window: &slint::Window) {
    window.with_winit_window(|window| window.set_maximized(!window.is_maximized()));
}

/// Give newly created auxiliary windows a predictable first position on Windows.
pub(crate) fn center_auxiliary_window(
    window: &(impl slint::ComponentHandle + 'static),
    owner: &(impl slint::ComponentHandle + 'static),
) {
    #[cfg(target_os = "windows")]
    windows::window_position::center_auxiliary_window(window, owner);
    #[cfg(not(target_os = "windows"))]
    let _ = (window, owner);
}

/// Center a top-level player window on Windows unless the user configured an
/// initial position. Other platforms keep their native window-placement policy.
pub(crate) fn center_player_window(
    window: &(impl slint::ComponentHandle + 'static),
    initial_position_configured: bool,
) {
    #[cfg(target_os = "windows")]
    if !initial_position_configured {
        windows::window_position::center_window(window);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, initial_position_configured);
}

/// Center a player opened from another player on the display that owns it.
/// This avoids sending a new player to the primary display when its parent is
/// on another monitor.
pub(crate) fn center_player_window_for_owner(
    window: &(impl slint::ComponentHandle + 'static),
    owner: &(impl slint::ComponentHandle + 'static),
    initial_position_configured: bool,
) {
    #[cfg(target_os = "windows")]
    if !initial_position_configured {
        windows::window_position::center_auxiliary_window(window, owner);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, owner, initial_position_configured);
}

/// Return the owning window's screen-space center for a child player process.
pub(crate) fn player_window_center(window: &slint::Window) -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        window
            .with_winit_window(|native| {
                let origin = native.outer_position().ok()?;
                let size = native.outer_size();
                Some((
                    origin
                        .x
                        .saturating_add(i32::try_from(size.width / 2).unwrap_or(i32::MAX)),
                    origin
                        .y
                        .saturating_add(i32::try_from(size.height / 2).unwrap_or(i32::MAX)),
                ))
            })
            .flatten()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// Read the taskbar-free monitor bounds for the native window on Windows.
pub(crate) fn window_work_area(
    window: &slint::Window,
) -> Option<riina_core::window_resize::ScreenRect> {
    #[cfg(target_os = "windows")]
    {
        window
            .with_winit_window(windows::window_position::work_area)
            .flatten()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// Return the current monitor's work-area limit for client content on Windows.
#[cfg(target_os = "windows")]
pub(crate) fn window_content_work_area(
    window: &slint::Window,
) -> Option<riina_core::window_aspect::PixelSize> {
    #[cfg(target_os = "windows")]
    {
        window
            .with_winit_window(windows::window_position::content_work_area)
            .flatten()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

/// Identify the aligned embedded video surface that is currently visible.
/// A replacement HWND restarts the Windows handoff stability interval.
pub(crate) fn native_video_surface_signature(window: &slint::Window) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        window
            .with_winit_window(windows::window_geometry::native_video_surface_signature)
            .flatten()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

pub(crate) fn system_trash_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Recycle Bin"
    } else {
        "Trash"
    }
}

/// Assign the stable Shell identity used by Windows automatic Jump Lists.
pub(crate) fn initialize_recent_documents() {
    #[cfg(target_os = "windows")]
    let _ = windows::recent_documents::initialize();
}

/// Add a locally opened media file to the platform's native recent documents.
pub(crate) fn remember_recent_document(path: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::recent_documents::remember(path)
    }
    #[cfg(target_os = "macos")]
    {
        unix::services::remember_recent_document(path)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = path;
        Ok(())
    }
}

/// Clear this application's native recent-document list where the OS supports it.
pub(crate) fn clear_recent_documents() -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::recent_documents::clear()
    }
    #[cfg(target_os = "macos")]
    {
        unix::services::clear_recent_documents()
    }
    #[cfg(target_os = "linux")]
    {
        Ok(())
    }
}

/// Uses each operating system's mounted-volume metadata for IINA's remote-drive policy.
pub(crate) fn is_remote_volume(path: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::services::is_remote_volume(path)
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        unix::services::is_remote_volume(path)
    }
}

/// Resolves native path aliases before shared media discovery.
pub(crate) fn resolve_alias_path(path: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        unix::services::resolve_alias_path(path)
    }
    #[cfg(target_os = "windows")]
    {
        windows::services::resolve_shortcut_path(std::path::Path::new(path))
            .map(|resolved| resolved.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_owned())
    }
    #[cfg(target_os = "linux")]
    {
        path.to_owned()
    }
}

/// Registers the checked media groups with the current platform's default-app service.
pub(crate) fn register_default_file_types(
    video: bool,
    audio: bool,
    playlists: bool,
) -> std::io::Result<usize> {
    #[cfg(target_os = "windows")]
    {
        windows::services::register_default_file_types(video, audio, playlists)
    }
    #[cfg(target_os = "macos")]
    {
        unix::services::register_default_file_types(video, audio, playlists)
    }
    #[cfg(target_os = "linux")]
    {
        unix::services::register_default_file_types(video, audio, playlists)
    }
}

/// Respect the Windows client-area motion accessibility switch without
/// mutating a user preference or making calls into the OS from a render callback.
pub(crate) fn system_reduce_motion_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::UI::WindowsAndMessaging::{
                SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
                SystemParametersInfoW,
            },
            core::BOOL,
        };

        let mut animations_enabled = BOOL(1);
        // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the valid local
        // output pointer; the zero flags value performs a read-only query.
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&mut animations_enabled as *mut BOOL).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok()
                && !animations_enabled.as_bool()
        }
    }

    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;

        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }

    #[cfg(target_os = "linux")]
    {
        unix::accessibility::system_reduce_motion_enabled()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

pub fn is_http_url(value: &str) -> bool {
    validate_browser_url(value).is_ok()
}

pub fn open_http_url_in_browser(value: &str) -> std::io::Result<()> {
    let url = validate_browser_url(value)?;
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::{PCWSTR, w},
        };
        use std::os::windows::ffi::OsStrExt;
        let url: Vec<u16> = std::ffi::OsStr::new(url.as_str())
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: the normalized URL is valid UTF-16 and remains live through
        // this call; one URL argument avoids shell interpretation.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(url.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(std::io::Error::other("Could not open the URL in a browser"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url.as_str())
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = url;
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Opening browser URLs is unsupported on this platform",
        ));
    }
    Ok(())
}

fn validate_browser_url(value: &str) -> std::io::Result<url::Url> {
    let Some((scheme, remainder)) = value.split_once(':') else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Only absolute HTTP and HTTPS URLs can open in a browser",
        ));
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Only HTTP and HTTPS playlist entries can open in a browser",
        ));
    }
    let has_authority = remainder
        .strip_prefix("//")
        .and_then(|value| value.split(['/', '?', '#']).next())
        .is_some_and(|host| !host.is_empty());
    if !has_authority || value.trim() != value {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Only absolute HTTP and HTTPS URLs can open in a browser",
        ));
    }
    let url = url::Url::parse(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Only HTTP and HTTPS playlist entries can open in a browser",
        ));
    }
    Ok(url)
}

#[cfg(test)]
mod browser_url_tests {
    use super::{is_http_url, validate_browser_url};

    #[test]
    fn browser_action_accepts_only_absolute_http_urls() {
        for value in [
            "https://example.com/watch?v=1",
            "http://localhost:8080/video",
            "HTTPS://EXAMPLE.COM/VIDEO",
        ] {
            assert!(is_http_url(value));
            assert!(validate_browser_url(value).is_ok());
        }
        for value in [
            "file:///C:/movie.mkv",
            "ftp://example.com/movie.mkv",
            "javascript:alert(1)",
            "https:///missing-host",
            "relative/path",
        ] {
            assert!(!is_http_url(value));
            assert!(validate_browser_url(value).is_err());
        }
    }
}

pub fn screenshot_directory(configured: Option<&str>) -> std::io::Result<std::path::PathBuf> {
    let directory = if let Some(path) = configured {
        if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
            let home = dirs::home_dir()
                .ok_or_else(|| std::io::Error::other("The home directory is unavailable"))?;
            if path == "~" {
                home
            } else {
                home.join(&path[2..])
            }
        } else {
            std::path::PathBuf::from(path)
        }
    } else {
        dirs::picture_dir()
            .ok_or_else(|| std::io::Error::other("The Pictures directory is unavailable"))?
            .join("Screenshots")
    };
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub fn open_directory(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::{PCWSTR, w},
        };
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: the owned directory string is NUL-terminated through the call;
        // the user's menu action opens the system file manager without a shell.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(path.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(std::io::Error::other("Could not open screenshot folder"));
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program).arg(path).spawn()?;
    }
    Ok(())
}

pub fn reveal_file(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::{
                System::Com::{
                    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                },
                UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
            },
            core::{HRESULT, PCWSTR},
        };
        use std::os::windows::ffi::OsStrExt;
        let text: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: COM is balanced only when this call acquired an initialization.
        // A pre-existing different apartment already satisfies Shell's COM need.
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let owns_com = initialized.is_ok();
        if !owns_com && initialized != HRESULT(0x80010106_u32 as i32) {
            return Err(std::io::Error::other(initialized.to_string()));
        }
        let mut item = std::ptr::null_mut();
        // SAFETY: the terminated path and output PIDL remain live. Zero selected
        // children means the absolute PIDL itself is the file to reveal.
        let result = unsafe {
            let parsed = SHParseDisplayName(PCWSTR(text.as_ptr()), None, &mut item, 0, None);
            let selected = parsed.and_then(|()| SHOpenFolderAndSelectItems(item, None, 0));
            if !item.is_null() {
                CoTaskMemFree(Some(item.cast()));
            }
            if owns_com {
                CoUninitialize();
            }
            selected
        };
        result.map_err(std::io::Error::other)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        // FileManager1 selection still needs a verified desktop integration;
        // opening the parent is the current explicit Linux adaptation.
        open_directory(
            path.parent()
                .ok_or_else(|| std::io::Error::other("Screenshot has no parent folder"))?,
        )
    }
}

pub(crate) fn save_playlist_file(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    content: String,
) {
    if app.get_dialog_open() {
        return;
    }
    app.set_dialog_open(true);
    app.invoke_activity();
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let saved = async {
            let owner = if let Some(host) = host.and_then(|host| host.upgrade()) {
                host.window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?
            } else {
                app.window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?
            };
            let file = rfd::AsyncFileDialog::new()
                .set_title("Save to Playlist")
                .set_file_name("playlist.m3u8")
                .add_filter("M3U8 playlist", &["m3u8"])
                .set_parent(owner.as_ref())
                .save_file()
                .await;
            if let Some(file) = file {
                file.write(content.as_bytes())
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<bool, String>(true)
            } else {
                Ok(false)
            }
        }
        .await;
        app.set_dialog_open(false);
        app.invoke_activity();
        match saved {
            Ok(true) => app.set_status("Playlist saved".into()),
            Ok(false) => {}
            Err(error) => app.set_status(format!("Could not save playlist: {error}").into()),
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod plugin_sidebar;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub(crate) use plugin_sidebar::PluginSidebarPresenter;
#[cfg(target_os = "linux")]
pub(crate) use plugin_sidebar::pump_linux_webview_events;
#[cfg(target_os = "windows")]
pub(crate) use windows::materials::native_materials_enabled;
#[cfg(target_os = "windows")]
pub(crate) fn windows_accent_color() -> Option<slint::Color> {
    let [alpha, red, green, blue] = windows::materials::native_accent_color()?;
    Some(slint::Color::from_argb_u8(alpha, red, green, blue))
}
#[cfg(target_os = "windows")]
pub(crate) fn windows_accent_foreground() -> Option<slint::Color> {
    let [_, red, green, blue] = windows::materials::native_accent_color()?;
    let linear = |channel: u8| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue);
    let foreground = if luminance > 0.179 { 0 } else { 255 };
    Some(slint::Color::from_argb_u8(
        255, foreground, foreground, foreground,
    ))
}
#[cfg(target_os = "windows")]
pub(crate) use windows::text_recognition;
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    NativePresentation, NativeVideoMaterialCapture, PluginOverlayPresenter, file_dialog,
    native_window, open_dialog, services::*, window_geometry::resize_content,
    window_sizing::WindowSizing,
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::text_recognition;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    ControlGesture, GesturePhase, MacApplicationMenuObserver, MacPictureInPicture,
    MacPluginMenuObserver, MacScrollEventMonitor, MacSubtitleColorPanel, RemoteMediaAction,
    SystemMediaControls, display_identity, display_name_for_path, icc_profile_for_window,
    install_control_gestures, set_represented_file, system_font_family_name,
};

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) use unix::{NativePresentation, open_dialog, services::*};
