# Add a plugin API

Match the pinned IINA API shape and capability requirements. The core bridge
registers JavaScript functions in
`crates/app/src/plugins/plugin_runtime/core_api.rs`. Read portable player state
through `crates/app/src/plugins/plugin_files.rs` and its `context.rs` snapshot.
Project confirmed engine state in `crates/app/src/plugins/plugin_snapshot.rs`.

For a read-only API such as `core.getVersion`, expose the bounded serialized
snapshot through a private bridge function, then remove that function from the
public object after capturing it.

```diff
--- a/crates/app/src/plugins/plugin_runtime/core_api.rs
+++ b/crates/app/src/plugins/plugin_runtime/core_api.rs
@@
+            const readVersion = core.__readVersion;
+            core.getVersion = () => JSON.parse(readVersion());
+            delete core.__readVersion;
```

Use the existing capability checks for restricted features. Validate plugin
arguments before queueing commands or performing I/O. Use typed player intents
for playback actions and keep the UI work on the UI thread. Add a focused
bridge test in `crates/app/src/plugins/plugin_runtime/tests.rs` for the public
result and any permission-dependent behavior. Run `cargo xtask check`.
