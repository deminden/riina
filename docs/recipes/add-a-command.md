# Add a playback command

Add the typed intent to `crates/core/src/command/mod.rs` and route it through
`Command::plan`. Put argument formatting in the corresponding domain module
under `crates/core/src/command/`. For an intent that needs observed state or
several native commands, add its route to `plan.rs`. Validate the complete
intent before issuing any part of the plan.

The existing `SetMuted` intent illustrates a command that needs one native
argument vector. `flag` preserves libmpv's `yes` and `no` spelling.

```diff
--- a/crates/core/src/command/mod.rs
+++ b/crates/core/src/command/mod.rs
@@
+    SetMuted(bool),
--- a/crates/core/src/command/arguments.rs
+++ b/crates/core/src/command/arguments.rs
@@
+            | Self::SetMuted(_) => self.audio_args()?,
--- a/crates/core/src/command/audio.rs
+++ b/crates/core/src/command/audio.rs
@@
+            Self::SetMuted(muted) => set("mute", flag(*muted)),
--- a/crates/core/src/tests/command_snapshot.rs
+++ b/crates/core/src/tests/command_snapshot.rs
@@
+    SetMuted[true];
```

Check `replaces_media`, `restarts_decoder` and `resets_playlist_shuffle` in
`command/mod.rs` for intents that affect worker lifecycle. For a command with a
special result, classify its reply in `crates/mpv/src/pending.rs` and process
it in `crates/mpv/src/worker.rs`. Bind player controls in the corresponding
feature module under `crates/app/src/player/player_ui/`.

Add one example to the exhaustive command inventory in `command_snapshot.rs`
and verify the emitted arguments against the pinned libmpv contract. Run
`cargo xtask check --fast` during development, then `cargo xtask check` and the
relevant `cargo xtask native` group.
