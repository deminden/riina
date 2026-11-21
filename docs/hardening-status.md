# Hardening review status

This table tracks findings from the November 17 review. The command and settings
golden tests use the repository's frozen IINA v1.5.0 behavior and preference
format. See [Verification](verification.md) for the scope of the runtime checks.

| Finding | Change | Local commits |
|---|---|---|
| H1 Preference recovery | Recover valid sections at startup, preserve the original bytes in quarantine, and report the reset sections. | `e63fce8`, `f90fffd` |
| H2 Startup diagnostics | Log fatal errors and panics and display startup failures on the native platform. | `f90fffd` |
| H3 Player worker and UI delivery | Report worker failure, retain native ownership through rendering teardown, and recover ordinary shared state after poisoned locks. | `acf3d30`, `e4525bd`, `fc2bdb7` |
| H5 Archive bounds | Bound extraction by bytes copied and recover abandoned staging directories. | `8316f0c` |
| H6 Network destination checks | Apply plugin grants and destination policy to the resolved addresses and each redirect. | `300495e` |
| H7 Native protocol | Name protocol values, assert ABI layouts, centralize observed properties, and keep raw client access inside the native handle owner. | `33e29dd`, `617f485`, `88c5619`, `8ed8c1a` |
| H8 Unsafe boundaries | Enforce documented unsafe blocks through workspace Clippy. | `dda8174` |
| H9 Plugin output routing | Use typed routes for plugin-driven effects and bounded output. | `7bcec50` |
| O4 App and UI crate boundaries | Test app callbacks through the library target and compile Slint in its own crate. | `acb0fbf`, `57346d1`, `721035b` |
| O6 Native worker ownership | Group pending replies, own the core in the event worker and share immutable playback snapshots. | `75eb5f4`, `8ed8c1a`, `2ef80b2`, `5802d69` |
| O7 Command planning and frame state | Separate argument and plan domains, share filter-chain planning and use typed frame slots. | `2f49a5c`, `8e9a1fb`, `ae692a8` |
