# Add a preference

An existing section owns its persisted values and its controls. For a boolean
in an existing restart-only section, edit the stored type, the Rust adapter,
and the Settings UI. The same section handles later edits and reopening.

## Example using the existing mpv OSD preference

1. Add the field and its default to the persisted section in
   [`crates/core/src/advanced_preferences.rs`](../../crates/core/src/advanced_preferences.rs).
   The Advanced section uses serde defaults so a partial profile fills in
   unspecified settings.

   ```diff
   +    pub use_mpv_osd: bool,
   +            use_mpv_osd: false, // in Settings::default
   ```

2. In [`crates/app/src/preferences/advanced_preferences.rs`](../../crates/app/src/preferences/advanced_preferences.rs),
   project the saved field in `install_window` and read it in
   `current_window`. Apply the value in `engine_options`, where the Advanced
   enable switch controls the native option. The section's
   `PreferenceSection::needs_restart` implementation supplies the restart
   notice for changes to Advanced settings.

   ```diff
   +    window.set_advanced_use_mpv_osd(settings.use_mpv_osd);
   +        use_mpv_osd: window.get_advanced_use_mpv_osd(),
   +        if settings.enabled && settings.use_mpv_osd { "1" } else { "0" }
   ```

3. In [`crates/ui/ui/preferences.slint`](../../crates/ui/ui/preferences.slint),
   bind a named `PreferenceSwitch` to the section's property. The existing
   Advanced callback forwards the edit to the common save path.

   ```diff
   +    in-out property <bool> advanced-use-mpv-osd: false;
   +    PreferenceSwitch {
   +        label: "Use mpv's OSD";
   +        enabled: root.advanced-enabled;
   +        checked: root.advanced-use-mpv-osd;
   +        toggled => {
   +            root.advanced-use-mpv-osd = !root.advanced-use-mpv-osd;
   +            root.advanced-settings-changed();
   +        }
   +    }
   ```

For preferences that update the player immediately, use the section's
`write_player` projection. Keep runtime-owned values such as the volume and
sidebar widths in the session capture.

Run `cargo xtask check --fast` while editing and `cargo xtask check` before
committing. Run `cargo xtask native` when an edit changes commands or options
sent to libmpv.
