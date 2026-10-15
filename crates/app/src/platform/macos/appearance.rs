use objc2_app_kit::NSFont;

/// Return AppKit's actual system font family so Slint can inherit native typography.
pub(crate) fn system_font_family_name() -> String {
    NSFont::systemFontOfSize(13.0)
        .familyName()
        .map_or_else(String::new, |family| family.to_string())
}
