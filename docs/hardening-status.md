# Hardening review status

This table tracks findings from the November 17 review. The command and settings
golden tests use the repository's frozen IINA v1.5.0 behavior and preference
format. See [Verification](verification.md) for the scope of the runtime checks.

From `906296f` to `0887a00`, the app's Rust sources went from 29 to 21
`TimerMode::Repeated` sites. The nine Slint timer declarations have
interaction-bound lifetimes. The application now creates production threads
through its named task owner. Direct thread creation elsewhere in the app
source tree is in test code.

| Finding | Change | Local commits |
|---|---|---|
| H1 Preference recovery | Recover valid sections at startup, preserve the original bytes in quarantine, and report the reset sections. | `e63fce8`, `f90fffd` |
| H2 Startup diagnostics | Log fatal errors and panics and display startup failures on the native platform. | `f90fffd` |
| H3 Player worker and UI delivery | Report worker failure, retain native ownership through rendering teardown, and recover ordinary shared state after poisoned locks. | `acf3d30`, `e4525bd`, `fc2bdb7` |
| H4 Core and engine error types | Return typed failures from the core and engine APIs, retain up to 16 playback notices, and preserve the latest message in the UI and plugin snapshot. | `47c8f73` |
| H4 Application diagnostics | Route formatted player statuses through one reporter. Log playback setup, native window and file cleanup failures. Release pending Windows menu and PiP state after rejected delivery. | `47c8f73`, `1dde0ee`, `a83c518`, `5aff11a`, `e887960`, `2df984f` |
| H5 Archive bounds | Bound extraction by bytes copied and recover abandoned staging directories. | `8316f0c` |
| H6 Network destination checks | Apply plugin grants and destination policy to the resolved addresses and each redirect. | `300495e` |
| H7 Native protocol | Name protocol values, assert ABI layouts, centralize observed properties, and keep raw client access inside the native handle owner. | `33e29dd`, `617f485`, `88c5619`, `8ed8c1a` |
| H8 Unsafe boundaries | Enforce documented unsafe blocks through workspace Clippy. | `dda8174` |
| H9 Plugin output routing | Use typed routes for plugin-driven effects and bounded output. | `7bcec50` |
| H10 Network identity | Share request defaults and versioned application identity across plugin clients. | `7bcec50` |
| H13 Development checks | Enforce strict Clippy, cache supported hosted jobs, add a non-blocking advisory job, and provide separate workspace, scoped, visual and native check commands. | `dda8174`, `9c7f48c`, `b3d8852`, `a73d9f5` |
| H14 Background task ownership | Name application workers, log panics, report running tasks at exit, centralize weak window delivery, and release discarded playback and rendering callbacks. | `e5f0dc9`, `475d58a`, `845f8c0` |
| H14 Preview and transition timers | Wake on playback, settings, or navigation events. Keep the 80 ms interval while work is pending and stop when it settles. | `17bd471` |
| H14 Idle media timers | Wake subtitle discovery and playlist metadata on state changes and worker results. Gate automatic subtitle lookup, Live Text and Windows video sampling by media and visibility. Keep Windows PiP polling active only for its preview or automatic minimize policy. | `9871d17`, `0887a00` |
| H14 Windows presentation timers | Pause alignment and native backdrop polling while the framed owner is hidden or minimized. Resume at the existing intervals on restore. | `223656d` |
| O1 Preference ownership | Store committed settings in one model, project them through section bindings, and capture runtime state separately when the player closes. | `5581847`, `33e7926`, `a83c518` |
| O2 Composition roots | Separate the Windows host and player session ownership, and split the history, crop and playback feedback handlers. Windows strict Clippy passed at the 150-line threshold. | `fc2bdb7`, `8bdb6e3`, `4684478` |
| O3 Slint structure | Keep the player and Settings roots under 600 lines, centralize color tokens, and preserve all 51 software-rendered Windows baselines. | `4bbdd65`, `954d4d6`, `0887a00` |
| O4 App and UI crate boundaries | Test app callbacks through the library target and compile Slint in its own crate. | `acb0fbf`, `57346d1`, `721035b` |
| O6 Native worker ownership | Group pending replies, own the core in the event worker and share immutable playback snapshots. | `75eb5f4`, `8ed8c1a`, `2ef80b2`, `5802d69` |
| O7 Command planning and frame state | Separate argument and plan domains, share filter-chain planning and use typed frame slots. | `2f49a5c`, `8e9a1fb`, `ae692a8` |
| D2 and D3 Duplicate command logic | Share audio and video filter commands and use one replay plan for ended playback. The command goldens cover both. | `8e9a1fb`, `ae692a8` |
| Test suite structure | Share temporary directories and snapshot waits, group native cases by subsystem, generate inputs for parsers and geometry, and inspect workflow structure. The optional coverage report has no threshold. | `502e61b`, `b7d0278`, `75852b8`, `0db351b`, `1ef80d2`, `975a51c`, `b018f68` |

| In progress | Verified progress | Local commits |
|---|---|---|
| H14 macOS media timers | Start PiP polling after AVKit controller creation and stop Now Playing updates before player cleanup. The Apple-target source check uses the generated player UI. Native playback and lifecycle verification require macOS. | `d8421ed` |
| O5 Platform boundary | Native player appearance, session services, track language names and wheel input translation use platform adapters. The plan's source scan counts 26 target conditionals outside `platform/`. Full macOS application verification remains open. | `7226679`, `e529719`, `47c8f73`, `204adaf`, `63c10a6` |
