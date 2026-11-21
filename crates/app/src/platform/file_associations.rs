//! The platform owns registration, native settings and worker lifetime for media associations.
use crate::PreferencesWindow;
use slint::ComponentHandle;

#[cfg(target_os = "linux")]
pub(crate) fn set_for_media(window: &PreferencesWindow, video: bool, audio: bool, playlists: bool) {
    let weak = window.as_weak();
    set_status(window, "Updating file associations…".to_owned());
    let worker = crate::background::spawn_named("riina-file-associations", move || {
        let message = match register(video, audio, playlists) {
            Ok(count) => default_app_status(count),
            Err(error) => format!("Could not register Riina file types: {error}"),
        };
        // A detached utility window no longer has a status line to update.
        let _ = crate::background::to_ui(weak, move |window| set_status(&window, message));
    });
    if let Err(error) = worker {
        set_status(
            window,
            format!("Could not start file-association update: {error}"),
        );
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn set_for_media(window: &PreferencesWindow, video: bool, audio: bool, playlists: bool) {
    let message = match register(video, audio, playlists) {
        Ok(count) => match open_default_apps() {
            Ok(()) => default_app_status(count),
            Err(error) => format!(
                "Registered {count} file types, but could not open Windows Default Apps: {error}"
            ),
        },
        Err(error) => format!("Could not register Riina file types: {error}"),
    };
    set_status(window, message);
}

fn set_status(window: &PreferencesWindow, status: String) {
    window
        .global::<riina_ui::UtilitySettingsState>()
        .set_utilities_status(status.into());
}

/// Registers the checked media groups with the current platform's default-app service.
fn register(video: bool, audio: bool, playlists: bool) -> std::io::Result<usize> {
    #[cfg(target_os = "windows")]
    {
        super::windows::services::register_default_file_types(video, audio, playlists)
    }
    #[cfg(not(target_os = "windows"))]
    {
        super::unix::services::register_default_file_types(video, audio, playlists)
    }
}

#[cfg(not(target_os = "linux"))]
fn open_default_apps() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:defaultapps")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open Windows Default Apps: {error}"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(())
    }
}

fn default_app_status(count: usize) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "Registered {count} file types. Choose Riina for the selected types in Windows Default Apps."
        )
    }
    #[cfg(target_os = "macos")]
    {
        format!("Launch Services set Riina as the default for {count} media types.")
    }
    #[cfg(target_os = "linux")]
    {
        format!("Riina is now the default for {count} selected media types.")
    }
}
