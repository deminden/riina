# Observe an mpv property

Register fixed playback observations in `crates/mpv/src/properties.rs`. The worker
and its event dispatcher both use the `OBSERVED` table. Each entry declares the
native name, payload type, projection and any cleanup when the value becomes
unavailable. Consult the pinned libmpv headers and player documentation for the
property's native format and availability rules.

## Example

The `deinterlace-active` property reports a native flag for confirmed activity.
It is distinct from the selected deinterlace option. The model in
`crates/core/src/playback.rs` stores it as `Option<bool>` so unavailable values
stay unknown.

```diff
--- a/crates/core/src/playback.rs
+++ b/crates/core/src/playback.rs
@@
+    pub deinterlace_active: Option<bool>,
--- a/crates/mpv/src/properties.rs
+++ b/crates/mpv/src/properties.rs
@@
+    property!(c"deinterlace-active", Flag, |state, flag| {
+        state.deinterlace_active = Some(flag);
+    }, |state| {
+        state.deinterlace_active = None;
+    }),
```

If the observation belongs in the interface, project it in
`crates/app/src/player/player_ui/observations.rs`. Preserve the distinction between
requested settings and native confirmation when updating controls and feedback.
The observer-table test checks that each name is unique and that applying a
representative value changes the state. Supply a sample for a new node property
and add a focused test for any additional conversion or clearing rule.

Run `cargo xtask check` and `cargo xtask native` after changing the table. The
`filters::` native subset covers the deinterlace example. Native rendering and
physical presentation have separate verification.
