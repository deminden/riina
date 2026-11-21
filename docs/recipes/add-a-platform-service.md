# Add a platform service

Expose the service through `crates/app/src/platform.rs` so feature code can call
the same Rust function on each target. Put native requests in the matching
`platform/windows/`, `platform/macos/` or `platform/unix/` adapter. Keep native
types, thread requirements and handle ownership inside that adapter.

For example, a native font query can live beside the system typography adapter
and be exposed by the facade. Return a portable value for an unsupported system.
An empty font family means Slint uses the backend default.

```diff
--- a/crates/app/src/platform.rs
+++ b/crates/app/src/platform.rs
@@
+#[cfg(target_os = "macos")]
+pub(crate) use macos::system_font_family_name;
+
+#[cfg(not(target_os = "macos"))]
+pub(crate) fn system_font_family_name() -> String {
+    String::new()
+}
```

For a service that performs an explicit user action, return an unsupported
error on platforms without an implementation so the caller can report it.
Preserve toolkit thread ownership and include a safety comment at native call
sites. Verify portable callers with workspace checks. Verify native behavior
with the platform-specific procedure in [Verification](../verification.md).
