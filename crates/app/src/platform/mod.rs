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
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(crate) mod desktop_drop;
pub(crate) mod file_associations;
mod file_reveal;
pub(crate) mod folder_dialog;
pub(crate) mod language;
mod pip_session;
pub(crate) mod plugin_dialogs;
pub(crate) use pip_session::PictureInPictureSession;
mod player_input;
pub(crate) mod playlist_menu;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod plugin_overlay;
mod plugin_page;
pub(crate) use player_input::{
    PlayerFocus, PlayerScrollMonitor, WinitWheelInput, batched_file_drops, file_drop_position,
    player_gesture_handler, winit_wheel_input,
};
mod plugin_presentation;
pub(crate) use plugin_presentation::{
    PluginMenuObservation, PluginOverlayDelivery, PluginPresenters, pump_plugin_webviews,
};
mod plugin_resources;
pub(crate) mod plugin_settings;
pub(crate) mod plugin_standalone;
pub(crate) mod preferences;
pub(crate) mod settings_file;
mod shared_video;
pub(crate) mod startup;
pub(crate) mod text_recognition;
pub(crate) use shared_video::SharedVideoExtras;
mod system_media_session;
pub(crate) mod transition_cover;
pub(crate) mod window_document;
pub(crate) use system_media_session::SystemMediaSession;
pub(crate) mod video_materials;
#[cfg(target_os = "linux")]
pub(crate) mod wayland_drop;
pub(crate) mod window_sizing;

#[cfg(any(target_os = "windows", test))]
pub(crate) fn update_hdr_display(app: &PlayerWindow, active: bool) {
    let video = app.global::<riina_ui::PlaybackVideoState>();
    if video.get_hdr_display_active() != active {
        video.set_hdr_display_active(active);
        // Display observations arrive independently of playback snapshots,
        // including while paused or after switching Windows HDR off.
        app.invoke_refresh_hdr_output();
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn show_fatal_error(title: &str, text: &str) {
    use ::windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::PCWSTR,
    };
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let text: Vec<u16> = text
        .replace('\0', "�")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: both UTF-16 strings are terminated and owned until the dialog returns.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn show_fatal_error(title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(text)
        .show();
}

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
                    crate::diagnostics::warn!(
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
                crate::diagnostics::warn!(
                    "Windows {title} window unavailable; keeping its solid surface: {error}"
                );
                return;
            }
        };
        let identity = match windows::materials::window_identity(&native) {
            Ok(identity) => identity,
            Err(error) => {
                crate::diagnostics::warn!("Could not identify the Windows {title} window: {error}");
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
                    crate::diagnostics::warn!(
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
        crate::diagnostics::warn!("Could not schedule the Windows {title} material: {error}");
    }
}

pub(crate) fn track_plugin_pointer(
    app: &crate::PlayerWindow,
    window: &slint::Window,
    event: &slint::winit_030::winit::event::WindowEvent,
) {
    use slint::ComponentHandle;
    if let slint::winit_030::winit::event::WindowEvent::CursorMoved { position, .. } = event {
        let scale = f64::from(window.scale_factor());
        let x = position.x / scale;
        let y = (f64::from(window.size().height) - position.y) / scale;
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_plugin_pointer_location(x as f32, y as f32);
    }
}

/// Use the host's click arbitration deadline before dispatching a single click.
pub(crate) fn double_click_interval() -> std::time::Duration {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: this system setting has no handle or pointer preconditions.
        std::time::Duration::from_millis(u64::from(unsafe {
            ::windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime()
        }))
    }
    #[cfg(target_os = "macos")]
    {
        std::time::Duration::from_secs_f64(objc2_app_kit::NSEvent::doubleClickInterval())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::time::Duration::from_millis(500)
    }
}

pub(crate) fn show_window(
    window: &(impl ComponentHandle + 'static),
) -> Result<(), slint::PlatformError> {
    window.show()?;
    #[cfg(target_os = "linux")]
    {
        unix::visibility::track(window);
        unix::visibility::request_wayland_first_frame(window.window());
    }
    Ok(())
}

pub(crate) fn hide_application() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Minimizing the taskbar owners also hides their video and controls.
        windows::bring_to_front::minimize_all_riina_windows()
    }
    #[cfg(target_os = "macos")]
    {
        let main_thread = objc2::MainThreadMarker::new()
            .ok_or_else(|| "Application hiding requires the main thread".to_owned())?;
        objc2_app_kit::NSApplication::sharedApplication(main_thread).hide(None);
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        unix::visibility::minimize_all();
        Ok(())
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

pub(crate) fn set_native_video_host_visible(
    host: Option<&slint::Weak<crate::NativeVideoHost>>,
    visible: bool,
) {
    let Some(host) = host.and_then(slint::Weak::upgrade) else {
        return;
    };
    // The native presentation retains this Winit handle for mpv's HWND
    // lifetime. Toggle the OS surface directly so Slint does not keep it shown.
    host.window()
        .with_winit_window(|window| window.set_visible(visible));
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
    if let Err(error) = windows::recent_documents::initialize() {
        crate::diagnostics::warn!(
            "Could not register the player for Windows recent documents: {error}"
        );
    }
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

pub(crate) fn configure_player_appearance(app: &PlayerWindow) {
    let appearance = app.global::<riina_ui::PlayerAppearance>();
    appearance.set_system_reduce_motion(system_reduce_motion_enabled());
    appearance.set_platform_macos(cfg!(target_os = "macos"));
    appearance.set_platform_windows(cfg!(target_os = "windows"));
    #[cfg(target_os = "windows")]
    {
        appearance.set_system_materials_enabled(native_materials_enabled());
        if let Some(accent) = windows_accent_color() {
            appearance.set_windows_accent_color(accent);
        }
        if let Some(foreground) = windows_accent_foreground() {
            appearance.set_windows_accent_foreground(foreground);
        }
    }
}

/// Read the platform motion preference outside the render callback.
pub(crate) fn system_reduce_motion_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::accessibility::system_reduce_motion_enabled()
    }

    #[cfg(target_os = "macos")]
    {
        macos::system_reduce_motion_enabled()
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
    open_shell_url(&url)
}

pub(crate) fn plugin_email_link(address: &str) -> Option<url::Url> {
    let (user, domain) = address.split_once('@')?;
    if user.is_empty()
        || domain.is_empty()
        || address.chars().any(|c| c.is_whitespace() || c.is_control())
        || domain.contains(['@', '/', '\\', '?', '#', ':', '%'])
        || user.contains([',', ';', '<', '>'])
    {
        return None;
    }
    let domain = url::Url::parse(&format!("https://{domain}")).ok()?;
    let host = domain.domain()?;
    let user = url::form_urlencoded::byte_serialize(user.as_bytes()).collect::<String>();
    url::Url::parse(&format!("mailto:{user}@{host}")).ok()
}

pub(crate) fn open_plugin_contact_email(value: &str) -> std::io::Result<()> {
    let url = plugin_email_link(value).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid author email address",
        )
    })?;
    open_shell_url(&url)
}

fn open_shell_url(url: &url::Url) -> std::io::Result<()> {
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
            return Err(std::io::Error::other(
                "Could not open the URL with its system handler",
            ));
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
    open_associated_path(path)
}

pub(crate) fn open_associated_path(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::{PCWSTR, w},
        };
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: the owned path is NUL-terminated and live through the call.
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
            return Err(std::io::Error::other(
                "Could not open the associated application",
            ));
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
    #[cfg(target_os = "linux")]
    return open_directory(
        path.parent()
            .ok_or_else(|| std::io::Error::other("The file has no parent folder"))?,
    );

    #[cfg(not(target_os = "linux"))]
    reveal_files(&[path.to_path_buf()])
}

pub(crate) fn reveal_files(paths: &[std::path::PathBuf]) -> std::io::Result<()> {
    file_reveal::reveal_files(paths)
}

pub(crate) fn save_playlist_file(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    content: String,
) {
    if app
        .global::<riina_ui::PlayerDialogState>()
        .get_dialog_open()
    {
        return;
    }
    app.global::<riina_ui::PlayerDialogState>()
        .set_dialog_open(true);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
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
                settings_file::save(file.path(), &content).map_err(|error| error.to_string())?;
                Ok::<bool, String>(true)
            } else {
                Ok(false)
            }
        }
        .await;
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_activity();
        match saved {
            Ok(true) => app.set_status("Playlist saved".into()),
            Ok(false) => {}
            Err(error) => {
                crate::diagnostics::report(&app, format!("Could not save playlist: {error}"))
            }
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
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
#[cfg(target_os = "macos")]
pub(crate) use macos::SessionExtras;
#[cfg(target_os = "linux")]
pub(crate) use unix::SessionExtras;
#[cfg(target_os = "windows")]
pub(crate) use windows::SessionExtras;
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    NativePresentation, NativeVideoMaterialCapture, file_dialog, open_dialog, services::*,
    window_sizing::WindowSizing,
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    ControlGesture, GesturePhase, MacPictureInPicture, MacPluginMenuObserver,
    MacSubtitleColorPanel, RemoteMediaAction, SystemMediaControls, control_gesture,
    display_identity, icc_profile_for_window,
};

#[cfg(target_os = "macos")]
pub(crate) use macos::system_font_family_name;

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_font_family_name() -> String {
    String::new()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) use unix::{NativePresentation, open_dialog, services::*};
