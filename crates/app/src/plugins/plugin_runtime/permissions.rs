//! The requested/granted intersection is checked before a realm is created.

#[derive(Clone, Copy, Default)]
pub(super) struct AvailablePermissions {
    pub(super) show_osd: bool,
    pub(super) show_alert: bool,
    pub(super) video_overlay: bool,
    pub(super) network_request: bool,
    pub(super) file_system: bool,
}

pub(super) fn reviewed_permissions(
    requested: &[String],
    granted: &[String],
) -> Result<AvailablePermissions, String> {
    let mut available = AvailablePermissions::default();
    for permission in requested {
        if !granted.iter().any(|grant| grant == permission) {
            return Err(format!(
                "Plugin permission {permission} has not been granted"
            ));
        }
        match permission.as_str() {
            "show-osd" => available.show_osd = true,
            "show-alert" => available.show_alert = true,
            "video-overlay" => available.video_overlay = true,
            "network-request" => available.network_request = true,
            "file-system" => available.file_system = true,
            _ => return Err(format!("Unknown plugin permission {permission}")),
        }
    }
    Ok(available)
}
