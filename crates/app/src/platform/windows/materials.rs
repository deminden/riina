//! DWM-backed Windows surface materials with an ordinary-surface fallback.

use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{
        DWM_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
        DWMWA_SYSTEMBACKDROP_TYPE, DwmGetWindowAttribute, DwmSetWindowAttribute,
    },
    System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        WindowsAndMessaging::{
            SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
};

const MATERIAL_POLICY_CACHE_TTL: Duration = Duration::from_millis(500);

thread_local! {
    static MATERIAL_POLICY_CACHE: RefCell<Option<(Instant, bool)>> = const { RefCell::new(None) };
    static MICA_POLICY_CACHE: RefCell<Option<(Instant, bool)>> = const { RefCell::new(None) };
    static ACCENT_COLOR_CACHE: RefCell<Option<(Instant, [u8; 4])>> = const { RefCell::new(None) };
}

/// Read the user's Windows accent color as ARGB for selected controls.
pub(crate) fn native_accent_color() -> Option<[u8; 4]> {
    ACCENT_COLOR_CACHE.with(|cache| {
        let mut cached = cache.borrow_mut();
        if let Some((updated_at, color)) = *cached
            && updated_at.elapsed() < MATERIAL_POLICY_CACHE_TTL
        {
            return Some(color);
        }

        let settings = UISettings::new().ok()?;
        let color = settings.GetColorValue(UIColorType::Accent).ok()?;
        let color = [color.A, color.R, color.G, color.B];
        *cached = Some((Instant::now(), color));
        Some(color)
    })
}

/// Follow Windows' Reduce transparency and Battery Saver policy for custom
/// video-backed materials. DWM also applies its own policy to native backdrops;
/// this keeps the renderer overlays from bypassing the same user choice.
pub(crate) fn native_materials_enabled() -> bool {
    MATERIAL_POLICY_CACHE.with(|cache| {
        let mut cached = cache.borrow_mut();
        if let Some((updated_at, enabled)) = *cached
            && updated_at.elapsed() < MATERIAL_POLICY_CACHE_TTL
        {
            return enabled;
        }

        let transparency_enabled = UISettings::new()
            .and_then(|settings| settings.AdvancedEffectsEnabled())
            .ok();
        let battery_saver_enabled = battery_saver_enabled();
        let high_contrast_enabled = high_contrast_enabled();
        let enabled = materials_allowed(
            transparency_enabled,
            battery_saver_enabled,
            high_contrast_enabled,
        );
        *cached = Some((Instant::now(), enabled));
        enabled
    })
}

/// Mica is a static wallpaper-backed base layer and remains enabled during
/// Battery Saver; unlike Acrylic, it does not keep sampling live content.
pub(crate) fn native_mica_enabled() -> bool {
    MICA_POLICY_CACHE.with(|cache| {
        let mut cached = cache.borrow_mut();
        if let Some((updated_at, enabled)) = *cached
            && updated_at.elapsed() < MATERIAL_POLICY_CACHE_TTL
        {
            return enabled;
        }

        let transparency_enabled = UISettings::new()
            .and_then(|settings| settings.AdvancedEffectsEnabled())
            .ok();
        let high_contrast_enabled = high_contrast_enabled();
        let enabled = mica_allowed(transparency_enabled, high_contrast_enabled);
        *cached = Some((Instant::now(), enabled));
        enabled
    })
}

fn materials_allowed(
    transparency_enabled: Option<bool>,
    battery_saver_enabled: Option<bool>,
    high_contrast_enabled: Option<bool>,
) -> bool {
    // Unknown platform policy must not leave a custom transparent effect active.
    transparency_enabled == Some(true)
        && battery_saver_enabled == Some(false)
        && high_contrast_enabled == Some(false)
}

fn mica_allowed(transparency_enabled: Option<bool>, high_contrast_enabled: Option<bool>) -> bool {
    transparency_enabled == Some(true) && high_contrast_enabled == Some(false)
}

fn battery_saver_enabled() -> Option<bool> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: the API writes the documented fixed-size status structure.
    unsafe { GetSystemPowerStatus(&mut status) }
        .ok()
        .map(|()| status.SystemStatusFlag == 1)
}

fn high_contrast_enabled() -> Option<bool> {
    let mut settings = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // SAFETY: the API writes the documented fixed-size structure. The returned
    // scheme pointer is owned by the system and is not retained or dereferenced.
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            Some((&mut settings as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()
    .map(|()| settings.dwFlags.contains(HCF_HIGHCONTRASTON))
}

/// Request the Windows 11 Mica system backdrop for the persistent player host.
///
/// Builds before Windows 11 22H2 can reject the attribute; callers keep the
/// normal renderer surface in that case rather than failing player startup.
/// A successful readback confirms the requested type, not visible material
/// pixels: Windows may show its system fallback under user or power policy.
pub(crate) fn apply_main_window_mica(window: &Window) -> Result<(), String> {
    request_backdrop(window, DWMSBT_MAINWINDOW, "Mica", native_mica_enabled())
}

/// Remove the persistent host backdrop while Windows transparency effects are
/// disabled; its Slint surface then returns to its solid fill.
pub(crate) fn clear_main_window_backdrop(window: &Window) -> Result<(), String> {
    set_backdrop_type(window, DWMSBT_NONE, "no system backdrop")
}

pub(crate) fn clear_system_backdrop(window: &Window) -> Result<(), String> {
    set_backdrop_type(window, DWMSBT_NONE, "no system backdrop")
}

pub(crate) fn window_identity(window: &Window) -> Result<isize, String> {
    hwnd(window).map(|handle| handle.0 as isize)
}

/// Request Windows 11 Desktop Acrylic for a transient auxiliary surface.
///
/// Builds before Windows 11 22H2 can reject the attribute; callers keep the
/// ordinary opaque surface in that case rather than failing the window.
pub(crate) fn apply_transient_window_acrylic(window: &Window) -> Result<(), String> {
    request_backdrop(
        window,
        DWMSBT_TRANSIENTWINDOW,
        "Desktop Acrylic",
        native_materials_enabled(),
    )
}

fn request_backdrop(
    window: &Window,
    requested: DWM_SYSTEMBACKDROP_TYPE,
    material: &str,
    policy_enabled: bool,
) -> Result<(), String> {
    if !policy_enabled {
        return Err(format!("Windows policy currently disables {material}"));
    }
    set_backdrop_type(window, requested, material)
}

fn set_backdrop_type(
    window: &Window,
    requested: DWM_SYSTEMBACKDROP_TYPE,
    material: &str,
) -> Result<(), String> {
    let hwnd = hwnd(window)?;
    // SAFETY: DWM reads this initialized enum synchronously and does not retain
    // its pointer. The HWND belongs to the live Winit window.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&requested as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .map_err(|error| format!("Could not set the Windows {material} backdrop: {error}"))?;

    let mut applied = DWM_SYSTEMBACKDROP_TYPE(0);
    // SAFETY: DWM writes one initialized enum-sized value for the live HWND.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&mut applied as *mut DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .map_err(|error| format!("Could not read back the Windows backdrop type: {error}"))?;
    if applied != requested {
        return Err(format!(
            "DWM reported backdrop type {}, expected {material} type {}",
            applied.0, requested.0,
        ));
    }
    Ok(())
}

fn hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("Windows Mica requires a Win32 window".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::materials_allowed;

    #[test]
    fn system_materials_follow_both_known_policy_values() {
        assert!(materials_allowed(Some(true), Some(false), Some(false)));
        assert!(!materials_allowed(Some(false), Some(false), Some(false)));
        assert!(!materials_allowed(Some(true), Some(true), Some(false)));
        assert!(!materials_allowed(Some(true), Some(false), Some(true)));
    }

    #[test]
    fn system_materials_fall_back_to_opaque_when_policy_cannot_be_read() {
        assert!(!materials_allowed(None, Some(false), Some(false)));
        assert!(!materials_allowed(Some(true), None, Some(false)));
        assert!(!materials_allowed(Some(true), Some(false), None));
        assert!(!materials_allowed(None, None, None));
    }
}
