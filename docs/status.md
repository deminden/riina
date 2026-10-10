Screenshot capture and folder opening now read live shared UI settings instead
of startup clones. Session saving reads the same validated typed settings, keeping
format/template/folder/subtitle/preview changes consistent across all three
paths. Unknown format or NUL-containing drafts cannot reach capture or overwrite
saved preferences. Platform-default versus custom directory remains an explicit
choice, rather than collapsing an empty draft into the default. This is shared
preferences plumbing; no preferences window or GUI interaction is claimed.

An original active ASS cue now has native saved-pixel evidence: video-only
versus subtitle captures differ in 1626 lower-frame pixels; hiding the selected
cue makes subtitle capture match video-only RGB pixels exactly. All three
captures retain pause and the confirmed 0.5-second position. The test must wait
for both cue text and seek position: cue text alone can arrive before the clock.
The native test and independent Pillow checker pass; workspace tests, formatting
and strict lint pass. Product code is unchanged, so the release was not rebuilt.
This verifies one primary ASS cue in SDR, not secondary/SRT/font/reference pixels,
GUI preferences or HDR. Preferences window and clipboard semantics remain open.
Changes and evidence remain local.

Screenshot settings now persist format (png/jpg/jpeg/webp/jxl/avif), naming
template, custom folder, subtitle inclusion and preview visibility. Old settings
retain PNG/%F-%n/platform Pictures/subtitles/preview defaults. The session save
retains these settings while updating volume. Core validates the whole capture
intent before native submission, with typed format choices and NUL rejection.
The app resolves a configured home-relative folder and uses it for capture and
folder opening. Preview-disabled captures use normal feedback without decoding
an accessory image. These are saved settings; a preferences interface is pending.

Native tests save all six format choices from original 4:2:0 media, reopen every
image in a separate native player and export decoded PNG frames. The pixel checker
confirms file signatures, 96x64 dimensions and expected two woven bar edges/widths
for all six. JPEG's alias correctly returns a .jpg filename. Reference SVT AVIF
encoder/still-picture settings are retained; original 4:2:2 AVIF input fails and
does not increment completion or replace the successful filename. No encoder
fallback or silent format conversion was added. JXL uses software screenshot
conversion; other formats restore native default policy.

An initial capture of reopened AVIF was requested before its clock/frame became
ready; the corrected test waits for playable phase and position as well as video
metadata. Native round trips then pass. A separate development PyAV16 JXL decode
exits 1 without an exception; its cause is unproven, so no independent JXL decoder
pass is claimed. Actual libmpv decoded-frame PNG checks pass. Unicode manifest
reading explicitly uses UTF-8.

Old-preference/default/custom round trips and invalid input checks pass, all 35
native tests pass (21.19 seconds), workspace tests, formatting and strict lint
pass; Windows release builds (46.30 seconds). Private AVIF template/reply check
also passes. GUI preferences/activation, active-subtitle composition, clipboard,
non-PNG preview fallback decoding, HDR/colorimetry and other OS runtime remain
unverified. Evidence: tests/visual/screenshot-formats-evidence.json. Nothing published.

Screenshot completion now drives a shared image preview with Screenshot Captured
feedback, five-second replacement timer, 300x200 aspect-preserving image bounds
and DELETE/EDIT/REVEAL accessory actions. The accessory uses the reference's
8-point top spacing, 36-point bottom region, 16-point buttons and 12-point gaps;
image border/radius are 1/4 points. Outer material, header stacking and typography
remain uncalibrated until rendered reference comparison. PNG preview failure
falls back to normal captured feedback. Source replacement, stop/failure phases,
crop entry and later video/seek messages retire the preview and its actionable
path. Completion consumes every snapshot independently of catalog caches; failed
captures do not increment or replay feedback, and a later confirmed success is
not masked by an unrelated stale general playback error.

DELETE attempts only the currently displayed native-returned file and hides the
accessory; EDIT opens its default system handler. Windows REVEAL uses an owned
absolute Shell PIDL and balanced COM initialization to select the exact file.
macOS uses open -R; Linux currently opens the parent folder and still needs
verified FileManager1 selection. No runtime action or deletion was automated.

Three meaningful checks pass: coalesced captures/retry/source retirement, landscape/
portrait/zero-size preview bounds, and actual Slint decoding of an original 16-bit
RGB PNG with expected patch colors/alpha. All 20 app unit tests, workspace tests,
formatting and strict lint pass. Windows optimized build passes (41.76 seconds).
Native engine is unchanged; its ignored suite was not rerun. No GUI activation,
five-second wall-clock timing, rendered pixels/materials, action outcome, shared
GL or other OS runtime pass is claimed. Evidence: tests/visual/screenshot-preview-evidence.json.
Nothing published.

Playback now exposes Take a Screenshot and Go to Screenshot Folder on both
player surfaces. The default action saves PNG with subtitles to the platform
Pictures/Screenshots folder, using native %F-%n naming. Command/Control+Shift+S
now requests capture, replacing the conflicting subtitle-panel binding; extra
modifiers, crop editing, dialogs and unavailable selected video retain guards.
The native owner forwards both actions to shared callbacks. Platform directory
resolution and file-manager opening remain outside core behavior. Screenshot
preferences, clipboard, five-second preview/accessory, complete Playback menu
and native accelerator display remain pending.

Native default capture returns the actual filename in its command reply. Success
counts update only after a successful current-generation reply with a filename.
A Unicode directory with an existing 0001 image produces 0002 then 0003 while
preserving the existing bytes, pause and position. An initial failure exposed
mpv appending '/' to verbatim Windows directories; retaining the platform-native
trailing separator fixes that path without stripping long-path prefixes. Explicit
video/subtitle PNG captures and no-frame failure still pass. All 35 native control
tests pass (20.06 seconds), workspace tests, strict lint and formatting pass, and
Windows release builds (54.17 seconds). A private logged template test also passes.

Windows native AX shows Playback before Video. F10/Return and screenshot shortcut
did not establish action completion; no catalog screenshot appeared. Clicking the
menu reported coordinate input geometry unavailable; screenshot capture reported
CreateForMonitor 0x80070057. Recovery found the player, but runtime menu/shortcut
activation remains unverified. Alt+F4 ends the process with exit 0 and only the
expected layout diagnostic. No pixels, preview, folder-open or other OS runtime
pass is claimed. Evidence: tests/visual/native-screenshot-defaults-evidence.json.
Nothing published.

Interlaced decoding investigation now compares original encoded MPEG-2 fields
with the v210 container-only fields. Both new fixtures regenerate byte-for-byte
with pinned development PyAV; all four clips pass the independent 75-to-150-frame
software oracle. Native default demuxing passes all 36 MPEG-2 captures with correct
field order and native flags. Across all 72 default captures, only six v210 BFF
On frames fail. A separate private diagnostic test selects libavformat: the same
v210 TFF/BFF files retain interlaced/field-order flags and all 36 saved captures
pass, including cadence and timestamp-aligned bar motion. This isolates a demuxer
path difference; the lower-level implementation defect and an IINA runtime
comparison remain unverified. No production default or UI changed. Preserve
the failing default-path regression while assessing a compatible remedy and
advanced-option parity. Evidence: tests/visual/interlaced-demuxer-comparison-evidence.json.
GUI/shared-GL/hardware/monitor and other OS application runtime remain pending.
Nothing published.

Native screenshot export now has typed video/subtitles/window content and
generation-scoped successful completion tracking. Failed or merely queued
requests cannot advertise a saved file. Native PNG tests verify no-frame failure,
96x64 output, completed PNG structure and unchanged pause/position. Screenshot
menus, shortcuts, preference formats/destinations, clipboard and window-mode
capture remain pending; this is engine groundwork, not completed screenshot parity.

The cadence test now saves 36 native video PNGs. The pixel checker passes all
24 woven Off captures and six top-first On captures, including bar widths and
timestamp-aligned motion. All six bottom-first On captures fail temporal order,
although their centre comb edges are removed. This is a retained failing
regression, not a verified deinterlacing-quality claim. Independent PyAV bwdif
orders the same source correctly. Native Matroska metadata handling is a candidate
for investigation; the precise cause remains unproven. Next compare an encoded
interlaced stream and demuxer behavior before changing production decoding.
Evidence: tests/visual/native-interlaced-pixels-evidence.json. These are saved
engine video pixels, not GUI/monitor screenshots. All 35 native control tests pass
(15.99 seconds), workspace tests, strict lint and formatting pass; Windows release
build passes (39.42 seconds). The separate image checker exits 1 with six failures.
Nothing published.

Original top-first and bottom-first interlaced fixtures now validate native
frame cadence: six steps each at 40 ms Off, 20 ms On, then 40 ms Off, with pause
preserved. An independent pinned PyAV decoder confirms 75 interlaced frames
and 150 correctly ordered progressive bwdif outputs per clip, with centre comb
edges removed. Native headless frame-info reports progressive flags even Off;
these approximate flags are not source classification evidence. Native rendered
comb removal, field order, hardware paths and current ports remain unverified.
All 34 native tests pass (14.46 seconds), workspace tests, formatting and strict
lint pass, and Windows release builds (43.51 seconds). All 13 fixture hashes
validate. Evidence: tests/visual/interlaced-cadence-evidence.json. Nothing published.

# Status · 2026-10-09

Video > Deinterlace now follows Flip on both player surfaces, defaults Off and
uses confirmed native state for its check and On/Off feedback. Native toggles
explicitly cycle only no/yes; current mpv's auto value remains unknown to this
two-state reference control. Configured deinterlacing and native automatic-filter
activity are separate, with source-scoped activity retired on replacement.
The new native test covers repeated explicit requests, rapid toggles, automatic
filter activity, crop/mirror coexistence, pause/volume/source geometry and source
replacement. It passes; all 33 native tests pass (13.37 seconds), as do workspace
tests, formatting, strict lint and release build (46.14 seconds). The fixture is
progressive; this proves filter/control behavior rather than comb removal or
interlaced-frame quality. Native GUI evidence verifies placement and On then Off
messages with Play/volume 37 preserved. Observing 200 ms after native teardown
also captures Aspect Ratio: Default after two separate activations, with the
first message absent before the repeat and unchanged position on the repeat.
This adds GUI feedback evidence beyond the earlier uncaptured aspect message.
Checks, exact timeout timing, rendered pixels, interlaced cadence, hardware paths,
shared GL and macOS/Linux runtime remain unverified. Clean close returns 0 with
only the expected layout diagnostic. Source/binary identity and observations:
tests/visual/native-deinterlace-menu-evidence.json. Nothing published.

Video > Aspect Ratio now precedes Crop/Rotation/Flip on both player surfaces,
with Default and the pinned nine menu ratios. Exact requested text belongs to
the playback worker and remains distinct from the observed native numeric
override: 8:6 selects neither 4:3 segment nor menu item; native Unicode parsing
failure retains requested text and reports the unchanged actual ratio/error.
The reference records requested text even on native failure. Invalid syntax
selects Default while aspect OSD reports the submitted text. Repeated requests
produce feedback even when the numeric property does not change. Request state
persists across source replacement, matching the player-wide aspect option.
Menu actions share the sidebar command path and reject crop/dialog/unknown
scopes. Formatting, strict lint, workspace tests and release build pass (40.62
seconds); all 32 native tests pass (12.77 seconds). Expanded geometry tests cover
all nine menu ratios plus panel 21:9 and custom 8:6; native repeat/failure/source
replacement checks and OSD unit checks pass. GUI evidence verifies menu order
and labels; 5:4 and Default activations recover a live player after transient
tool reference loss, preserving Play/volume 37. The one-second OSD was not
captured; exact GUI ratios/checks/pixels, timed OSD and ports remain unverified.
Clean close returns 0 with only the expected layout diagnostic. Source/binary
identity and observations: tests/visual/native-aspect-menu-evidence.json. Nothing pushed.

Video > Rotation now exposes the pinned four degree labels between Crop and
Flip on shared and native player menus. Selection derives from observed native
rotation; menu callbacks validate loaded/known state and crop/dialog scopes,
renew the native menu model and dispatch the existing portable rotation intent.
No additional shortcut or optimistic selection is introduced. Formatting,
strict lint, workspace tests and Windows release build pass (42.95 seconds).
The native geometry test passes (0.17 seconds) for 90/180/270/0 with pause,
volume 37, aspect reset and track identity preserved. Native GUI testing verifies
the hierarchy and ordered labels; 90° activation loses the input tool reference,
then fresh inventory recovers the responsive player at Play/volume 37/position
4.999979. The Video sidebar opens; checks and rendered rotation are not exposed
by this accessibility view. Clean close returns 0 with only the expected layout
diagnostic. Exact GUI output, no-op/failure checks, shared GL and ports remain
unverified. Aspect Ratio menu remains pending exact requested-text provenance
and its reference OSD. Source/binary evidence:
tests/visual/native-rotation-menu-evidence.json. Nothing published.

Crop and Flip menu activations now renew menu model identity before dispatch or
input-scope rejection. The pinned muda backend automatically toggles native
check items before delivering actions; the Slint backend rebuilds them only when
tracked properties change. Renewing the model reasserts confirmed playback
selection even when a command makes no state change. Reflection callbacks now
also reject unknown/unloaded state and active crop/dialog scopes. Workspace
tests, formatting, strict lint and Windows release build pass (37.14 seconds).
Native GUI testing observed menu recreation after None activation, successful
Custom… entry and Cancel restoration with Play/volume 37/position 5 preserved.
Native checks are still not exposed by the accessibility view; checked pixels,
failure/burst behavior, shared GL and other-platform runtime remain unverified.
The player closes with exit 0 and only the expected native layout diagnostic.
Source/binary identity and observations: tests/visual/native-video-menu-refresh-evidence.json.
Nothing published.

Native Crop menu evidence on source e6b97ed verifies the complete ordered ratio
labels and direct Custom… editor entry. Up wraps from None to Custom…; End did
not move focus in this test. Cancel restores all standard window controls and
Play/volume 37/EOF position 5. A 4:3 attempt loses the input tool's window
reference; fresh inventory recovers closed menus and player input, but does not
prove its filter result. Close returns 0 with only the expected diagnostic.
Evidence/source/binary identity is in tests/visual/native-crop-menu-evidence.json.
The separator and checks are not exposed by AX. No-op native check-item
resynchronization and named intent for dimension collisions need further work.
Release build passes (39.25 seconds). Workspace tests, formatting and strict
lint pass; all 32 native tests pass (12.89 seconds), including all ten crop ratios.
Nothing published.

Video > Crop now exists on the shared player and native owner, with the pinned
menu's distinct nine ratios, None, separator and Custom… entry. It routes through
the existing typed crop command/editor and shares observed catalog state; panel
presets remain separate. Decimal cinema ratios use exact integer fractions.
The native crop replacement test now covers all menu ratios plus panel 21:9,
preserving pause/volume/source geometry and navigation provenance. It passes;
workspace tests and strict lint pass after fixing a test initializer lint.
Geometry collisions on tiny sources now leave selection unknown rather than
checking an arbitrary ratio; preserving named intent in that case remains open.
GUI menu checks/actions, Custom… entry and pixels are pending verification.
Nothing published.

Native crop window-control evidence on source d9bc274 shows Close disabled and
Minimise/Maximise absent from the accessible title bar during editing. Return
confirms the whole-source selection and restores all three controls enabled,
with Play/volume 37/EOF position 5 preserved. Alt+F4 then closes with exit 0 and
only the expected layout diagnostic. Source/binary identity and limits are in
tests/visual/native-crop-window-controls-evidence.json. This establishes Windows
availability/restoration, not IINA's visual disabled-button appearance. Cancel,
source retirement, shared GL/macOS runtime, document-icon and Linux equivalents
remain unverified. Workspace tests, strict lint, formatting and release build
pass (43.92 seconds). Nothing published.

Crop editing now disables standard close/minimize/maximize buttons on the shared
player and, when present, its native video owner. Exiting restores each window's
previous button mask. The independent shared coordinator uses winit's portable
window API, with no Windows handles in crop state. This follows the pinned
InteractiveMode.swift entry/exit button behavior; document-icon handling and
Linux window-manager equivalents remain open. Strict lint passes; runtime
disabled/restored control and lifecycle verification is pending. Nothing published.

Native source 2c60229 retains ordinary single-F10 menu entry and shortcut
recovery. In Custom Crop, F10 leaves the editor active; subsequent keyboard
selection of 4:3 and Escape work, preserving Play/volume 37/EOF position 5.
In the native file picker, F10 retains picker focus; cancellation restores the
Video shortcut and playback observations. Close returns 0. Evidence/source/
binary identity is in tests/visual/native-menu-input-scope-evidence.json.
An external application-switch attempt exposes retained AX focus in both apps,
so it does not prove foreground retention; queued transition races also remain
unverified. Workspace tests, strict lint, formatting and release build pass
(36.05 seconds). Nothing published.
API basis for the foreground gate: [GetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow).

Native menu requests now recheck crop/dialog input scope when the queued entry
and system command execute. A transition between activation and entry clears
menu ownership and returns input to the crop editor without disturbing a dialog.
Menu-exit recovery now requires the foreground window to remain this owner's
host/chrome pair and skips modal dialogs, preventing recovery from reclaiming
another application's input. Strict lint passes; editor/dialog and external
activation runtime verification is pending. Nothing published.

Native queued-entry source 8a517fe now has runtime evidence for single-F10/Return
entry and repeated Video > Flip navigation. Right exposes both exact reflection
labels. Returning on Mirror loses the tool's window reference; fresh inventory
recovers a live player with menus closed, chrome focused, Play and volume 37
preserved, and working Video shortcut. This proves recovery, not the filter
result. Escape closes the dropdown to menu-bar scope, then another Escape exits
the bar and restores shortcuts. Direct bar cancellation needs one Escape and
also restores the Video shortcut. Close returns 0 with only the expected layout
diagnostic. Source/binary identity and limits are recorded in
tests/visual/native-menu-queued-entry-evidence.json. Checked state, reflected
pixels, pointer/DPI/client geometry and ports remain unverified. Nothing published.
After these changes, all 32 native playback tests pass (12.89 seconds), workspace
tests pass, strict lint passes and the release build passes (36.14 seconds).

Owner activation inside SC_KEYMENU still left first F10 on the owner
(source 9c04399, binary
61f0489a7e180e0cd19336e6378fce0e1276f2687e044090e9537d0d92efac16).
Return did not open Video; close returned 0. Activation and menu entry are now
separate queued Windows messages, allowing activation to finish before the
system command. The entry generation invalidates older queued recoveries.
Strict lint passes; revised entry/recovery verification is pending. Nothing
published.

Direct owner F10 routing still required another press in the runtime check
(source e9c576e, binary
211d844f8b39ef5ed21d53b1a3d6eca5861ab971ffabafa8ec2d0e6cf9c8b506).
Temporary tracing showed the first SC_KEYMENU reaching the owner without
WM_ENTERMENULOOP, while a later request entered the loop and Return opened
Video > Flip. The player closed with exit 0. Entry now explicitly activates
the framed owner before assigning focus; strict lint passes, runtime verification
is pending. Traces were removed. No public update.
API basis: [SetActiveWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setactivewindow).

Native source 15cbd57 restores shared chrome after Escape and the subsequent
Video shortcut opens its sidebar; the player closes with exit 0. First F10 still
focuses only the owner, and Return does not open Video. Evidence/binary identity
is in native-menu-loop-recovery-evidence.json. The framed owner now also handles
F10 key release directly through the same queued entry, covering input that
never reaches shared controls. Strict lint passes; revised entry remains pending
runtime verification. No public update.

The deferred native menu revision (source 12057a9, binary
92e9a507004ded136b919f6a33d9cf6f8dab542c3de5e9f7a094ad38967c7484)
still returned to chrome on first F10 and closed with exit 0. Temporary message
tracing then showed owner menu entry followed by automatic chrome refocus.
The Windows winit focus helper also synthesizes Alt. Menu-loop ownership now
persists across owner focus events, uses direct same-thread SetFocus, and queues
entry/recovery through Windows messages rather than Slint timers. Registered
recovery messages carry a generation so an older exit cannot reclaim input from
a later menu loop. Temporary traces were removed. Workspace tests and strict
lint pass; revised first-entry, cancellation/action recovery and lifecycle need
runtime verification. Nothing published.
API basis: [SetFocus](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus),
[menu entry](https://learn.microsoft.com/en-us/windows/win32/menurc/wm-entermenuloop),
[menu exit](https://learn.microsoft.com/en-us/windows/win32/menurc/wm-exitmenuloop).

The key-release-only native menu revision still returned focus to chrome on its
first F10 in a runtime check (source f8aff94, binary e965dcc289cf49b72d83f11b86a04011d31307b51d741d3b8301810917e6ff76).
That player closed with exit 0. Owner focus/menu entry is now deferred until the
next event-loop turn, after default key-up handling. Strict lint passes; the
deferred handoff remains pending runtime verification. Nothing published.

Native menu focus testing found two remaining issues on source 6d9d181: first
F10 focuses the owner but does not retain menu focus; second F10 reaches the
menu bar. Escape leaves owner focus and the Video shortcut does not reopen its
sidebar. Evidence/binary identity is saved in native-menu-focus-timing-evidence.json.
F10 handoff now occurs on key release, and a retained UI-thread window subclass
defers chrome focus restoration after WM_EXITMENULOOP, including cancellation.
Compilation and strict lint pass. Revised entry/recovery and subclass lifetime
still require runtime verification; the tested build closed with exit 0. No
public update. The pre-fix release build passed in 38.25 seconds.
Workspace unit tests and formatting pass after the changes.

Native menu runtime exposed a real focus gap on source 42552b5: the owner has
Video in its native menu bar, but F10 remains in the transparent shared chrome.
The tested build closes cleanly; its identity/failure limits are recorded in
tests/visual/native-video-menu-entry-evidence.json. F10 now requests the owner's
Windows menu loop, suppressing one automatic chrome refocus, and selected menu
actions restore chrome focus. Workspace unit tests, strict lint and formatting
pass after the fix. Fixed entry/action behavior and Escape/cancel focus recovery
still need runtime checks; no passing fixed GUI result is claimed. No publication.

The experimental native video route now attaches Video > Flip to its framed
owner window. Shared controller observations synchronize menu availability and
axis checks, and owner menu actions invoke the existing shared toggle callback.
This keeps one playback model while separating framed native menus from the
transparent controls popup. A duplicate Slint loaded-change callback found at
compilation was merged with the existing focus callback. Compilation and strict
lint pass; native menu input/checks, client geometry/focus, lifecycle and pixels
still require runtime verification. No GUI/full-native-suite rerun/publication.
Workspace unit tests and formatting pass.

Windows native application-menu evidence now covers Video > Flip, both exact
labels, F10/Return/Right navigation and Mirror activation returning to player
scope. Play and volume 37 remain observed; EOF position shifts slightly to
4.9999585 during decoder refresh. Check marks are not exposed by this AX tree,
so visual checked state is unverified. A later action lost its window reference;
fresh inventory/state recovered the live player and its result remains unknown.
Close returns 0 with empty stderr. Evidence/source/binary limits are recorded in
tests/visual/video-reflection-menu-evidence.json. Release build passes in 38.07
seconds. No reflected pixels, Vertical activation or port runtime claim.
Nothing published.

The shared player now exposes Video > Flip > Horizontal (Mirror)/Vertical (Flip)
through Slint's menu bar, with observed filter checks and native atomic toggle
callbacks. Entries require a known video/filter catalog and are disabled during
crop editing. No default key equivalents or dedicated OSD were introduced.
The experimental native video route excludes this menu until attachment to its
owner window is implemented. Other reference menus, configurable bindings,
actual menu input/check behavior, reflected pixels and port runtime remain open.
Workspace unit tests, strict lint and formatting pass. No publication.

Flip/mirror now also have native atomic toggle commands. Passing the full
managed filter specification to vf toggle adds/removes the filter; label-only
toggle would merely change its enabled flag. This keeps rapid activations from
using a stale UI catalog. Native coverage passes for single on/off and an
interleaved two-axis burst, retaining crop, pause, volume and navigation count.
The pinned menu items have no default key equivalents, and their handlers do
not emit a dedicated flip/mirror OSD; do not invent either. Menu and configurable
IINA-command binding integration, reflected pixels and OS runtime remain open.
No GUI/full-native-suite rerun and no public update.
Workspace unit tests, strict lint and formatting pass.

Flip/mirror now have portable explicit-state commands with independently owned
hflip/vflip labels. Unknown filter state is rejected; repeated enabled/disabled
requests are idempotent and removal touches only the requested axis. Native
coverage verifies both axes together, individual removal, existing 4:3 crop,
pause and volume 37 preservation, and no navigation feedback. The focused test
passes. This is backend/filter-catalog evidence, not reflected-pixel proof or a
user-facing feature: the reference exposes these in Video > Flip; application
menu/shortcut/OSD wiring and other OS runtime remain open. Nothing published.
Workspace unit tests, strict lint and formatting pass; no GUI or full native
suite rerun for this change.

Additional native coverage verifies zero-width, zero-height and both-zero custom
crops at offset (10,12). Each retains the exact requested filter parameters,
settles after a raw decoder restart with full 320×180 output, preserves pause
and volume 37, and does not add navigation feedback. All three are accepted;
they do not crop just the nonzero dimension. The initial expected partial output
failed and was corrected from observed native state. The focused regression now
passes, as do strict lint and formatting. No product code changed in this step.
Actual empty-selection pointer/rendered reference comparison remains open;
no GUI test, whole native-suite rerun or publication for this evidence increment.

Interactive crop drafts now retain zero extents through projection, integer
conversion and explicit confirmation. The native rectangle command forwards
zero dimensions, while strict nonempty filter validation remains available for
ordinary geometry. Native evidence verifies crop=0:0:0:0 keeps 320×180 output,
reports Custom and resets cleanly. All 30 native tests pass in 12.61 seconds;
workspace unit tests, strict lint and formatting pass. Regressions retain the
existing nonempty integer-selection contract and cover empty continuous drafts.
An initial tiny-source regression exposed edge snapping bypassing the integer
empty-drag guard; the guard now checks clamped endpoints before conversion.
Actual mouse/empty-overlay rendering, offset zero-size native outcomes, edge
crossing and reference GUI comparison remain open. No GUI test/public update.

Crop preset selection now remains retained control state after pointer edits
and editor re-entry, matching the pinned reference's persistent segmented
control. Riina previously cleared it in both paths. Whole-source initialization
on entry remains separate from that retained selection. This is source/UI
wiring evidence; actual selected highlight and pointer/re-entry runtime remain
unverified. The empty-selection path also differs: the reference submits its
integer crop without a zero-size guard, whereas riina rejects zero-area drafts
or native confirmation. Resolve native zero-dimension semantics and rendered
reference behavior before claiming that edge case. No GUI/native rerun here;
no public update.
Workspace unit tests, strict lint and formatting pass.

The current native-window crop build now has Windows keyboard/lifecycle evidence:
Custom opens at whole source, 4:3 labels (40, 0) (240×180), Escape restores player
scope with Play/position 5/volume 37, and close returns 0. This run was at EOF;
it does not prove playing-source restoration. Stderr contains only the expected
scale/client/layout diagnostic. Binary/source identity and verification limits
are saved in tests/visual/native-crop-layout-keyboard-evidence.json. Native
picture pixels, shadow, pointer behavior, Done, resize and retirement remain
unverified. No public update.

Current shared-GL crop layout/shadow build has Windows keyboard evidence on
source a69ab1e: Custom opens at whole source, 4:3 produces (40, 0) (240×180),
and Escape restores settled Play/position 2/volume 37 with player scope focus.
Clean close returns 0 with empty stderr. Binary hash and limits are recorded in
tests/visual/crop-layout-keyboard-evidence.json. The release build passes in
33.26 seconds. No picture/blur/pointer proof, current Done verification or native
window GUI evidence is claimed. Nothing published.

Native crop exit now queues margin reset before the crop-exit command that can
restore playing state. This removes an ordering gap in which resumed frames
could briefly retain editor margins. All 29 native tests pass in 12.38 seconds
on the preceding viewport implementation; that suite verifies engine behavior,
not this app coordinator's visual ordering. After the coordinator change,
workspace unit tests, strict lint and formatting pass. Actual native-window
Done/Cancel playback and rendered transitions still need GUI verification.
No public update.

Native crop editing now queues four normalized video margins from the same
interactive picture projection used by the shared renderer/selection overlay.
Viewport changes update margins; ending or retiring the editor queues zero
margins. Portable validation rejects invalid/out-of-window rectangles and keeps
DPI out of the ratios. A new native test passes for margin application, resize
and reset command acceptance, preserving pause, volume, raw source dimensions
and navigation counters. This is headless option acceptance, not native-window
pixel or lifecycle proof. Workspace unit tests, strict lint and formatting pass.
Native shadow, actual alignment/input and current other OS runtime remain open;
no GUI test and no public update.
Optimized Windows build passes in 36.62 seconds.

Shared OpenGL crop mode now draws a shadow behind the padded picture, with
10-point blur, black at approximately 50% alpha and zero offset, matching the
pinned reference's explicit parameters and NSShadow initialization defaults
([Apple documentation](https://developer.apple.com/documentation/appkit/nsshadow/init%28%29)).
Slint compilation, workspace unit tests, strict lint and formatting pass.
This does not prove matching blur kernels or rendered pixels. No GUI/native
suite rerun; native viewport/shadow wiring and other OS runtime remain open.
No public update.

The shared OpenGL route now renders its crop-editing picture into the reserved,
top-anchored interactive bounds and projects the selection through those same
bounds. Rendering dimensions are separate from the full window/editor viewport,
so returning from editing restores the normal full-area render target. Native
window presentation remains on the previous full-area geometry pending native
viewport wiring; shadow, actual pixels, pointer operation and port runtime remain
unverified. Workspace unit tests, strict lint and formatting pass. No GUI/native
suite rerun and no publication for this wiring change.
Optimized Windows build passes in 37.63 seconds.

A portable interactive-picture projection now encodes the reference's 20-point
top/horizontal inset and 108-point height reservation, with top anchoring after
contain-fit. Regression coverage checks numeric landscape/rotated layouts,
fractional source round-trips and invalid viewports. This is a layout foundation,
not a visible UI change: both presentation routes still require coordinated
viewport wiring, shadow and rendered verification. Workspace unit tests, strict
lint and formatting pass. No GUI or native-suite rerun; no publication.

Tiny crop selections now retain standardized vertical resize hit strips when
their height is below four logical points. Previously those strips collapsed
to zero, unlike the pinned reference's rectangle normalization. Shared Slint
compilation, workspace unit tests, strict lint and formatting pass. Actual
pointer targeting and rendered comparison remain unverified; no GUI or native
suite rerun was performed for this layout correction. Changes remain local.

Crop edge dragging now preserves the pointer's grab offset inside each resize
strip. Source inspection exposed an absolute-pointer mismatch that could jump
an edge; the portable drag model now applies deltas from the initial grab.
Regressions cover all four edges with off-center grabs, unchanged initial
selection and fixed opposite edges. Free reverse dragging and invalid pointers
are also covered. The shared UI now uses four-point edge hit strips with
top/bottom corner priority, directional resize cursors and a two-point outline.
Workspace unit tests, strict lint and formatting pass. This is model/UI wiring
verification; actual pointer operation, tiny/crossed edge behavior, rendered
parity and current other OS runtime remain open. No GUI or native-suite rerun
was performed for this change. No public update.
Optimized Windows build passes in 37.48 seconds.

The crop editor now retains continuous source coordinates for pointer selection,
edge resizing, presets and viewport reprojection. Integer conversion is confined
to label/confirmation boundaries, preserving truncation and source validation.
New checks show a 0.15-logical-unit motion changes the projected overlay while
the integer label remains unchanged, and fractional drafts survive quarter-turn
rotation/resize without integer drift. Existing integer projection callers and
all 28 native tests pass; workspace unit tests, strict lint and formatting pass.
Actual pointer/overlay rendering, reference padding/shadow, titlebar controls,
native candidate layout and current other OS runtime remain unverified. No GUI
test was run for this model change. No public update.
The optimized Windows build passes in 36.40 seconds.

Custom crop is now connected to a shared editor with whole-source selection,
six presets including 3:2, source-coordinate label, Done/Cancel and free/edge drag
handlers. Entry waits for native pause/filter reset; stale source observations
retire the draft. Resize reprojects source pixels. Editor keys suppress playback
shortcuts and retain Tab navigation. Windows shared-GL GUI at 4e1724e verifies
Custom entry, whole 320×180 selection, 4:3 label (40,0)/(240×180), Return
confirmation and Escape cancellation with Play/position 2/volume 37 retained.
Reset readiness temporarily disables presets/Done and then enables them.
Close exits 0 with empty stderr. Evidence: tests/visual/crop-editor-evidence.json.
Source-retirement focus restoration and entry projection validation were added
after that GUI run. Pointer handlers, cropped pixels, continuous subpixel draft,
IINA video-area padding/shadow, native-video-candidate layout, playing GUI pause
restoration, decoder conflicts and current port runtime remain unverified.
Final workspace unit checks, strict lint, formatting and optimized Windows build
pass (34.09 seconds). The later focus/entry correction was not GUI-retested.
No public update.

Crop selection now has portable source/display projection for contain-fit
letterboxing, display aspect and clockwise quarter turns. Pointer selection
clamps to actual content, handles reverse drags, snaps within four source pixels
of edges and preserves integer rectangles across resize cycles. The first
round-trip test exposed a one-pixel floating-point truncation error; only
machine-error distance from integer pixels is now corrected. Tiny sources retain
whole-frame selections and empty drags stay invalid. Observed display dimensions
and rotation feed projection without inventing defaults for unavailable values.
Native coverage verifies mapping under three aspect choices and four rotations.
This is selection geometry; Custom... remains disabled until the editor surface,
handles and controls are connected. Actual rendered/IINA coordinate parity,
native route layout, other filters and current port runtime remain open.
All 28 native tests, workspace unit tests, strict lint, formatting and optimized
Windows build pass (35.23 seconds).
No public update.

Interactive crop now has a portable editing-session lifecycle tied to source
generation and dimensions. Entry clears the managed crop and explicitly pauses;
confirmation applies the validated rectangle and restores prior pause state,
while cancellation only restores pause and leaves the entry-cleared crop absent.
Native coverage checks Done/Cancel from playing and paused sources, volume 37,
and rejection of stale confirmation after same-file replacement without
changing the replacement's pause or filters. This is backend lifecycle work;
the shared Custom... control remains disabled until the editor exists.
Selection overlay, source/display coordinate mapping, confirmation/cancel UI,
native route layout, crop OSD, pixel parity and current port runtime remain open.
All 27 native tests, workspace unit tests, strict lint, formatting and optimized
Windows build pass (34.70 seconds).
No public update.

The shared video sidebar now includes the crop row between aspect and rotation,
with None and five working preset commands. Selection maps observed native
filter parameters to presets; unknown state stays unknown and explicit
coordinates display Custom. Custom... is disabled pending its interactive
editor. Initial build exposed borrowed-state and status-setter wiring errors;
both are corrected. Unit checks, strict lint, formatting, the expanded focused
native crop test and optimized Windows build (33.86 seconds) pass.
Windows accessibility verifies row/focus order, Tab from custom aspect into crop,
arrow/Space input without changing pause/position 2/volume 37, and skipping the
disabled Custom control into rotation. Close exits 0 with empty stderr.
Evidence: tests/visual/crop-sidebar-evidence.json. Selected segment was not
exposed by the reader; actual GUI crop pixels/effect are not verified by it.
Native selection mapping is tested separately. Custom interaction, crop OSD,
small-source ambiguous preset geometry, mixed foreign filters, decoding conflict
policy, rendered parity and current macOS/Linux runtime remain open. No public update.

Crop now also accepts explicit source-pixel rectangles with dispatch-time bounds
validation. Confirming the whole source clears only its managed crop. Native
coverage exercises all five presets under a 1:1 aspect override at every quarter
turn, retaining source dimensions and pause/volume/rotation/aspect, plus an
offset custom rectangle and whole-frame reset. The shared selection/confirmation
UI is still missing; native mixed foreign-filter chains, decoding conflicts,
presentation parity and current macOS/Linux runtime remain open. Nothing published.
The expanded focused native test, workspace unit checks, strict lint and
formatting pass; the full 26-test native suite also passed before the final
whole-frame confirmation refinement.
The final optimized Windows build passes in 32.60 seconds.

Portable crop foundations now provide typed presets, source-pixel geometry,
managed-label replacement/removal and observed native video-filter catalogs.
The native regression verifies all five presets, one managed crop after each
replacement, unchanged source dimensions, pause/volume preservation and reset.
Its first run exposed YUV420 alignment: requested 320×137 produces 320×136;
the test now checks both requested parameters and actual aligned output.
Foreign-filter reset isolation has command-plan coverage, not native mixed-filter
coverage. Controls, interactive custom cropping, hardware-filter conflict policy,
crop feedback, source parity and current other OS runtime remain open.
All 26 native tests, workspace unit tests, strict lint, formatting and optimized
Windows build pass (34.34 seconds).
No public update.

The selected-track last-cue case also reproduces stale feedback: requesting the
next cue at position 3 makes a later resumed A–B loop count as navigation.
Explicit pause/resume now retires unstarted feedback while retaining a seek
that has already started. New native and worker regressions cover that boundary.
All 25 native tests, workspace unit tests, strict lint, formatting and optimized
build pass. No-op requests during already-running loops, first
cue boundaries, rapid mixed-command timing, GUI/reference presentation and
current other OS runtime remain open. No public update.

Native testing reproduces stale navigation feedback: sub-seek with no selected
subtitle arms a token that an automatic A–B loop consumes. The worker now only
arms subtitle-navigation feedback when a primary subtitle selection is observed;
the native command is still submitted. All 24 native tests pass after repair.
Forward/backward navigation with selected captions also increments navigation
completions. Workspace unit checks, strict lint, formatting and optimized build
pass. Selected-track cue boundaries, selection/command races, GUI retest
and current other OS runtime remain open. Nothing published.

OSD preferences now separately suppress speed and seek messages, both false by
default. Legacy preferences retain visible feedback; JSON round-trip preserves
each flag. Speed suppression consumes confirmed changes without replaying them
and retains equalizer/hardware messages, including a coalesced speed/decoding
change covered by unit regression. Isolated Windows GUI at cbee705 verifies
speed 2 with no speed message, seek position 2 with no clock, and Brightness:
-27 still visible. Saved flags remain true; close exits 0 with empty stderr.
Evidence: tests/visual/osd-suppression-evidence.json. Updated unit checks and
strict lint, formatting and final optimized build pass. Settings-page
switches, locale text, simultaneous coalesced-message ordering and current
other OS runtime remain open. No public update.

Seek feedback now uses a distinct completed-navigation counter associated with
submitted seek/cue/chapter/frame commands, while the raw completed native seek
counter retains decoder refreshes. Worker feedback clears on replacement,
decoder-changing intent and failed reply. Native testing exposes reply-before-
SEEK ordering; successful replies now retain provenance until playback completes.
Regression checks cover internal restart suppression and repeated same-position
navigation. All 23 native tests, workspace unit tests, formatting, strict lint
and optimized build pass. Shared-GL and native-video-candidate GUIs show Off/On
for decoding toggles, then clock feedback for deliberate navigation, with Play
and volume 37 retained. Both close with exit 0; GL stderr is empty and native
stderr has only the existing chrome diagnostic. Evidence:
tests/visual/decoding-feedback-evidence.json. Asynchronous mixed-command stress, rendered parity and current other
OS runtime remain open. Nothing published.

Video-setting OSD now derives messages from confirmed native speed, configured
hardware decoding and five equalizer channels, with signed equalizer progress.
Initial GUI showed brightness feedback but missed speed messages. The catalog
cache did not consume settings-only updates; repair gives feedback independent
observation history, with a regression test for sequential brightness/speed
changes and clock-only suppression. Repaired Windows GUI confirms Brightness:
-27 then Speed: 1.234567x and eventual expiry while retaining Play and volume
37. Close exits 0 with empty stderr. The hardware toggle still exposes the seek
clock instead of retaining its decoding message; decoder-refresh seek feedback
must be distinguished from deliberate navigation. Evidence:
tests/visual/video-feedback-evidence.json. Workspace unit tests, strict lint,
formatting and optimized build pass.
Initial/unknown values and generation changes do not create feedback; replacement,
stop and errors hide it. Existing global OSD enable/timeout controls apply, and
video and seek messages exclude each other. Locale messages, speed-specific
disable preference, exact coalesced ordering and rendered OSD materials/geometry
and current other OS runtime remain open. Nothing published.

Custom speed entry now submits finite above-100 requests to libmpv instead of
inventing an upper UI boundary absent from IINA's formatter. A native check
confirms 101 reports an error and preserves speed 1.25, pause and volume 37.
Very large finite input remains finite through six-decimal normalization.
All 22 native tests, updated core unit checks, formatting, strict lint and
optimized build pass. Windows reset retest shows reset reachable at 2x, absent
from AX at 1x and skipped by Tab, which reaches Hardware Decoding. Keyboard
clear / Return also restores blank entry to 1x. Pause, volume 37 and position 5
remain; close exits 0 with empty stderr. Evidence:
tests/visual/speed-reset-focus-evidence.json. Equivalent native error
presentation, rendered pixels and current port runtime remain open. Changes
stay local.

The video panel now includes a logarithmic speed slider, precise custom entry
and reset hidden at 1x, ordered before hardware decoding and equalizer controls.
Shared Rust conversion spans 25 ticks from 0.25x to 16x, rounding slider rates
to two decimals; custom entry keeps six decimals without passing through f32.
Blank input resets 1x. Unit checks and all 21 native tests pass, as do optimized
build, strict lint and formatting. Windows GUI confirms 1.2345675 submission
refreshes to 1.234567 and Tab / Return reset restores 1 and slider position 8;
Play, volume 37 and position 5 remain. Close exits 0 with empty stderr. The reset
node remains in AX after reset, so actual hidden visibility remains unverified.
Evidence: tests/visual/video-speed-evidence.json. Locale formatting, fractional rounding equivalence, above-100
requests, OSD, measured control rendering and current port runtime remain open.
No public update.

Video equalizer controls now expose brightness, contrast, saturation, gamma and
hue in reference order, with independent reset, integer range -100–100 and a
neutral marker. Portable typed commands reject out-of-range values; observed
unknown channels disable their controls. All 20 native tests pass, including
independent channel edits and resets that preserve pause, volume 37 and speed
1.5. Optimized build, unit checks, strict lint and formatting pass. Windows GUI
exposes all five zero-valued channels; brightness -27 returns as observed -27,
and Tab / Return on its reset returns zero with other channels zero, Play,
volume 37 and position 5 retained. Close exits 0 with empty stderr. Evidence:
tests/visual/video-equalizer-evidence.json. Rendered controls, OSD,
actual color-output comparisons and current macOS/Linux runtime remain open.
Changes remain local.

Hardware decoding now has a shared sidebar switch and typed portable Disabled /
Auto / AutoCopy policies. The observed configured policy drives the switch;
hwdec-current remains a distinct active-decoder diagnostic. Preference default
is Auto and the legacy settings format retains that default. Toggle off/on
restores the preferred mode without overwriting it. All 19 native tests pass,
including policy changes while paused at volume 37 and preservation across file
replacement. Optimized Windows build, unit checks, formatting and strict lint
pass. GUI Tab reaches the switch after the four rotation choices; Space and
Return retain the Play button and volume 37, with clean exit 0 and empty stderr.
AX omits checked state, so actual mode changes are established by the separate
native test. Evidence: tests/visual/hardware-switch-evidence.json. Native
switch rendering, Settings mode picker, OSD, accelerated codec coverage and
current macOS/Linux runtime remain open. No public update.

The full native run exposes an intermittent pre-existing secondary-text
assertion failure; five isolated repeats pass. Its same-position seek wait can
accept stale cue text, so the test now waits for an observed seek completion
before hiding the lane, retaining its original assertion. Three consecutive
full native suites now pass all 18 tests. Optimized build, formatting and strict
lint pass. No broad stability or full-parity conclusion is claimed.

Video labels now include source dimensions and rounded frame rate; tests verify
the native catalog label 320×180, 24fps stays unchanged under aspect/rotation.
Custom ratios preserve Unicode decimal digits; a native test verifies original
Arabic-digit text fails in libmpv without changing the previous ratio, pause or
volume. Supplementary-digit acceptance is source-inferred and requires macOS
verification. The optimized Windows shared-OpenGL GUI verifies the complete
320×180, 24fps track label and closes with exit 0 and empty stderr; evidence is
in tests/visual/track-label-evidence.json. Pixel comparison, fractional-rate GUI
fixtures and current other OS runtime remain pending. Changes remain local;
public updates are reserved for substantial verified milestones.

Video custom-entry Tab repair is verified on f3cfed8: Tab reaches rotation;
Right/Left do not seek a paused 2-second timeline or change volume 37. Normal
close exits 0 with empty stderr. AX cannot expose selected segments, so actual
aspect/output-ratio and rotation values remain backed by the separate native
test, not a pixel claim. Evidence: video-settings-repair-evidence.json. Strict
lint and release build pass; all 17 native tests passed on unchanged backend.
Rendered/native control parity and current other OS runtime remain open. Local
commits only; nothing published.

Initial video GUI at 9a9d1ad confirms the controls/labels and custom edit, with
clean shutdown, but Tab is trapped in the custom field. Repair passes Tab
through, exposes selectable segment state and routes Left/Right to video choices
instead of timeline seeks. Runtime retest is pending. Reference/native segmented
focus semantics and rendered geometry remain open; no public update.

Video aspect/rotation and video track selection are implemented in the portable
intent/backend and shared trailing panel. Native tests verify six aspect presets,
custom ratios, invalid-input Default reset and quarter-turn properties without
changing pause/volume. Exact ratio text distinguishes 8:6 from the 4:3 preset.
Two new core checks and all 17 native tests pass. Actual GUI interaction/rendering,
OSD, segmented keyboard parity, crop and remaining video sections, and current
macOS/Linux runtime remain open. No public update was made.

Repaired Windows source 3b81c9d verifies readable audio and both subtitle language
labels on shared OpenGL and experimental native-video routes. Both close with
exit 0; GL stderr is empty and native has only its existing chrome diagnostic.
Exact binary hashes and scoped observations are in language-badges-repair-evidence.json
and language-badges-native-evidence.json. Formatting, strict lint, unit checks,
all 16 explicitly enabled native tests, release build, fixture hashes and
traceability pass. Pixel/name equivalence, context-loss/resume, lifecycle stress
and current macOS/Linux runtime remain open. Nothing was published.

Actual Windows accessibility labels at 7622217 verify audio and both subtitle
language lists, but closing exits 101: a teardown property update re-enters
Slint's mutable window borrow. Original diagnostic evidence is retained in
language-badges-initial-evidence.json. The lifecycle repair clears the frame
before normal hide and defers context-loss UI changes with a setup-generation
guard. Runtime shutdown retest is pending. No pixel pass or public update.

Readable audio/primary/secondary subtitle language badges now use locale names,
including ISO three-letter aliases and script/region identifiers. macOS uses
Foundation directly; Windows/Linux use pinned Unicode data behind a portable
presentation boundary. Three unit checks and an original seven-track libmpv
fixture pass. Backend omits und; absent metadata has no badge. Exact native-name
equivalence, experimental formatter locale patterns, GUI rendering and other OS
runtime remain unverified. Full parity remains incomplete; work stays local.

Observed-face restoration guard verified on 5d865e7: filtered Verdana family,
face Down→Regular and Return commit are followed by reopening with custom name
Verdana and accessible Regular focus intact. Earlier first-face overwrite is
not reproduced in this sequence. Default font was restored, retaining size 55,
pause and volume 37; diagnostic process exits 0 with empty stderr. Exact binary
hash and scoped observations: tests/visual/font-rows-selection-restore-evidence.json.
Formatting, strict lint, unit checks and optimized release build pass. The earlier
deferred-focus binary verifies first focus and End/Home/Page navigation with no
panic. Double-click is implemented with native toolkit events but pointer testing
is still unavailable; no activation or pixel pass is claimed. Retained-row
performance, focus timing, native-video and other OS runtime remain open.
All work stays local.

Deferred-focus runtime on ae7f7c5 succeeds: first family Tab, End/Home and
PageDown/PageUp report the corresponding row after settling, without the earlier
missing-ID panic. Filter replacement to Verdana and face Tab/Down report Italic
then Regular; Return commits Verdana with size 55, pause and volume 37 preserved.
Reopening then exposed a new race: focus selected first-face Italic before the
queued Regular restoration. Automatic focus selection is now suppressed while
reveal/restoration is pending; its runtime retest remains pending. Default font
was restored and the diagnostic process exited 0 with empty stderr. Exact
binary findings: tests/visual/font-rows-deferred-focus-evidence.json. Pointer
double-click, rendered layout, performance and other OS checks remain open.
All work stays local.

Retained rows alone did not fix the accessibility panic: be545eb exits 101 on
first family Tab, with another missing focused-ID panic. Scoped diagnostics:
tests/visual/font-rows-retention-failure-evidence.json. Focus delegation now
falls back to the stable list during model/navigation changes, then publishes
the row after a 32ms toolkit tree-update interval. This is an independently
implemented synchronization repair, not a completed accessibility claim.
Strict lint passes; first-focus, boundary, model replacement and pointer runtime
retests remain pending. Retained-row memory/performance and exact focus timing
also remain open. All changes stay local.

Font row boundary regression reproduced on 893e8f5: filtered family/face focus
reports Verdana/Italic/Regular correctly, but full-catalog focus reports the
container. End changes the draft to the last family; Home then terminates the
player. A diagnostic reproduction exits -1073740791 and records AccessKit
consumer 0.29.0 panic: Focused ID is not in the node list. Exact pre-repair
observations and binary hash: tests/visual/font-rows-focus-windows-evidence.json.
Rows now use ScrollView with retained row instances instead of virtualization;
focus delegation also rejects absent indices. This preserves IDs when scrolling
across the catalog. Strict lint passes; repaired boundary/focus runtime remains
pending. Catalog memory/performance, pointer double-click and rendered metrics
remain open. All work is local.

Original shared font rows now expose native toolkit double-click activation.
Double-clicking a face refreshes its exact font identity before accepting; family
double-click does not accept. Selection and keyboard cursor share a bounded row
index, with Home/End/Page/arrow navigation and reveal. Rows expose accessible
labels/selection and default selection action. The list remains virtualized;
first build rejected additional ListView children, so focus and background now
sit outside its single repeater. Formatting, strict lint, unit checks and release
build pass. Initial Windows GL runtime on ca9000d verifies filtered Verdana
family Tab selects its first face, face Down selects Regular, and Escape Cancel
preserves sans-serif, size 55, pause and volume 37. Accessible focus incorrectly
reported the window, because the keyboard focus scope was outside the list node.
The list role/focus delegate now sit on their common parent; row text no longer
adds duplicate accessible labels. Strict lint passes for the repair; its runtime
retest is pending. Double-click injection failed with coordinate input geometry
is unavailable; no activation pass is claimed. Exact pre-repair binary findings:
tests/visual/font-rows-initial-windows-evidence.json. Player windows were closed.
Scrolling, double-click runtime and rendering remain pending.
19pt pitch is derived from recorded scroll increments, not a proven native row
height; colors/type/focus are provisional. Other OS runtime remains open.
All work stays local.

Font chooser source-layout correction: preview now precedes custom entry,
matching the pinned XIB, with Choose a font, Typeface, Preview and Or enter the
font name labels. Recorded margins are 12pt; table block 268pt including 23pt
headers; search/custom inputs 24pt; preview 36pt; buttons 80pt with 12pt gap.
Return now invokes acceptance from the filter, preview or list scope; Escape
remains Cancel. Formatting, strict lint, unit checks and release build pass.
Windows shared GL runtime on 10d6623 verifies label/accessibility order and Return
acceptance from preview, filter and restored Regular face, preserving size 55,
pause and volume 37. Default font was restored and settled player windows closed.
Exact binary hash and scoped evidence:
tests/visual/font-picker-return-windows-evidence.json. Rendered geometry/clipping,
modified Return, native-video and other OS retests remain pending.
The XIB's contentRect 524pt versus contentView
496pt discrepancy is unresolved; 524pt remains and bottom flexible space absorbs
the difference. Toolkit table materials/rows/fonts are still provisional.
Face double-click acceptance is separately inventoried and unimplemented; the
toolkit list exposes pointer events without click count, so a timing guess was
not introduced. Native-route retained-state and other OS runtime remain open.
All changes stay local.

Reusable font chooser state is now retained after OK, Cancel and native close.
Preview text and filter persist in the same component/catalog; each invocation
refreshes custom entry and matching face from observed playback, including when
already visible. This corrects the earlier draft-preservation claim for the font
entry itself: only preview/search survive, while selection is refreshed. A scan
may populate the hidden reusable chooser without showing it; shutdown drops it.
Pinned AppDelegate's single chooser and Utility's selection-before-show define
the contract. Strict lint, unit checks and release build pass. Windows shared GL
retest on 37c4623 verifies Unicode sample and Verdana search retained across
Escape Cancel, accepted custom-entry Enter and native Alt+F4 close/reopen.
Cancelled custom font is replaced by observed sans-serif on reopening; accepted
Verdana restores its face list. Defaults were restored, with size 55, pause and
volume 37 unchanged; settled shutdown leaves no player windows. Exact binary hash
and scoped AX evidence: tests/visual/font-picker-reuse-windows-evidence.json.
Close-before-scan race and already-visible font refresh remain untested.
Application-wide reuse across multiple players/Settings, native-video reuse and
other OS runtime remain open. No rendered sample or pixel parity is claimed.
All work stays local.

Native font chooser lifecycle verified on ebf6d9c: chooser now appears as a
separate targetable Choose a Font window instead of being nested under the video
host. Parent Left seeks the paused five-second fixture from its endpoint to zero
with volume 37 retained; the chooser and Native draft café preview stay open.
Repeated opening reuses the same chooser and draft. Activating the parent then
Alt+F4 dismisses both windows; settled inventory and process inspection are empty.
Exact binary hash and scoped AX observations:
tests/visual/font-picker-native-lifecycle-evidence.json. Formatting, strict lint,
workspace unit checks and optimized release build pass. This proves the narrow
native candidate behavior only; no pixels or calibrated HDR pass is claimed.
Other OS runtime, global reusable picker and Settings sheets remain open.
All changes are local.

Native-route auxiliary window isolation: the winit attributes hook now consumes
the pending host owner for exactly the next adapter (the video chrome). Previously
it remained installed and applied transparent/non-resizable/taskbar-hidden owner
attributes to every later auxiliary window. Pinned Slint backend inspection confirms
the hook runs at adapter creation. Strict lint and unit checks pass; corrected
release runtime retest is pending. Pre-fix native chooser was nested under the host
in Windows accessibility inventory; native window styling and parent interaction
were not verified. The pre-fix chooser and player were closed. All work is local.

The player font chooser now follows the pinned ordinary-window behavior rather
than setting the modal file-dialog gate. Repeated opening raises the existing
live draft, and player shutdown dismisses it in both shared GL and Windows native
routes. Its title now matches Choose a Font. Source contract: PlayerCore
chooseSubFont calls Utility quickFontPickerWindow without a sheet owner; the
new Settings page supplies an owner and is a separate pending requirement.
Strict lint, unit checks and release build pass. Windows shared GL runtime on
80b4000 verifies parent font size Down 55→60 and Up→55 while the chooser stays
open; repeated opening reuses the same chooser and preserves Keep draft café.
Closing the parent dismisses the chooser and settled inventory has no riina
windows or process. Scoped binary-hashed evidence is in
tests/visual/font-picker-lifecycle-windows-evidence.json. One parent set-value
attempt hit the automation cache; fresh activation and keyboard navigation
worked. Native-video route and other OS runtime, global application-wide reuse
and Settings sheet remain open. No native ownership or pixel pass is claimed.
All work stays local.

Repaired font chooser runtime verified locally on source 321225e. Accessibility
set-value changes Test to Preview café Привет. Committing Verdana and reopening
now exposes Verdana in the family viewport; Tab into the face list focuses
Regular without replacing the draft. Family Tab exposes the list container,
so its precise keyboard cursor is still unproven. Default sans-serif was restored;
size 55, paused playback and volume 37 remained unchanged, and both windows closed.
Evidence: tests/visual/font-picker-repair-windows-evidence.json, including the
exact release binary hash. Release build succeeds. This is accessibility evidence,
not rendered glyph or pixel parity. Native modality, platform ordering/metrics
and other OS runtime remain open. Nothing was published.

Local font preview/face GUI test on 0a5f367: real keyboard editing changes Test
to a Unicode sample and retains it during family/face changes. Verdana selection
shows readable Italic/Regular/Bold Italic/Bold rows; Down selects Regular and OK
commits Verdana, preserving pause, volume 37 and size 55. Reopening restores the
face list and draft, but the family viewport still begins at Arial: reveal failed
after layout settled. Accessibility set-value only focused the preview instead
of editing it. Exact binary findings are recorded in
tests/visual/font-face-preview-windows-evidence.json; default font was restored
and both windows closed. No rendered glyph/pixel proof is claimed.

Repairs now defer reveal until both virtualized lists have measured content,
cancel stale reveals on search/user face changes, and disable work after chooser
closure. The preview now implements accessibility set-value explicitly. Strict
lint and all unit checks pass; repaired-source runtime retest remains pending.
Native modality, platform face ordering and other OS checks remain open.
All work stays local.

Local font chooser reveal wiring: restoring a matching face now uses the shared
list's selection API, which brings the row into view and updates its keyboard
cursor rather than changing only the highlighted index. A restoration guard
suppresses user-choice callbacks, preserving the custom name, filter and preview.
Choosing a family similarly reveals its first face through the guarded API.
Strict lint and unit checks pass. Offscreen row visibility, initial Tab behavior
and rendered scrolling still require a GUI retest; no runtime scrolling pass
is claimed. Inventory has 103 atomic rows and remains incomplete. All work is local.

Local font preview fidelity: the chooser now has the source-defined editable
single-line centered Test sample, 24px type and 36px source-height field. Preview
uses the selected family's name plus weight/italic metadata instead of treating
an arbitrary custom/PostScript identifier as a toolkit family. Empty/filtered-out
selection resets to the system font; preview edits have no font-command callback.
Strict lint and unit checks pass. This is source-derived layout and compiled
wiring, not rendered font equivalence. GUI editing/Enter behavior, preview text
lifetime, condensed/variable faces, fallback glyphs and exact native metrics
remain open. Inventory now contains 102 atomic requirements and remains
incomplete. No public update was made.

Local font face labels: chooser rows now show readable typographic/subfamily
labels while exact PostScript names remain selection and playback identities.
Pinned ttf-parser 0.25.1 reads existing font metadata during the background scan;
missing/invalid labels fall back to the identity without copying font files.
Windows probe still discovers 72 families/166 faces, now with readable labels
for all 166 faces in 127 ms. Only counts are saved. Strict lint and unit checks
pass; tests cover name-table priority, Unicode, invalid/control labels and
corrupt font input. This is metadata evidence, not a rendered chooser or glyph
pass. Locale-specific labels, platform face ordering, chooser visual verification
and other OS runtime remain open. Changes stay local.

Local font picker selection fidelity: choosing a family now selects its first
face and fills custom entry with that face's PostScript name, matching the
pinned selection behavior. Opening or filtering restores a visible exact face
match without changing custom entry or the search. Generic/custom names without
a matching installed face stay unselected. A focused unit check covers filtered
row identity, Unicode, exact case, hidden matches and generic names; all unit
checks and strict lint pass. New GUI routing/highlights and platform face order,
scroll-to-selected behavior, preview and other OS runtime remain unverified.
Inventory now contains 100 atomic rows and remains incomplete. Changes stay local.

Local font chooser runtime retest on 8f86c69: Windows filtering to Arial leaves
one family; keyboard family focus fills five face rows and updates the draft.
A no-match filter clears both lists. Escape closes the chooser, restores button
focus and leaves sans-serif unchanged. Custom-entry Enter commits monospace;
reopening shows that observed name, and clearing/Enter restores sans-serif.
Size 55, pause and volume 37 remain unchanged. Scoped evidence and exact binary
hash: tests/visual/font-picker-edit-windows-evidence.json. Release build passes.
This tests control routing, not loaded subtitle rendering or preview glyphs.
Individual face choice, OK button, native ownership/modality, localized sorting,
pixel parity and other OS runtime remain open. Nothing was published.

Local font chooser keyboard/selection follow-up: Escape now invokes Cancel
through the chooser focus scope. Family and face current indices are explicitly
cleared before a filtered model replacement, so an old numeric row cannot stay
highlighted as a different family. Choosing a new family also clears the old
face highlight. Strict lint, formatting and all unit checks pass. GUI retesting
of Escape and filtered selection remains pending; the older cancellation
evidence proves only Cancel-button behavior. No public update was made.

Local shared font chooser: the subtitle Font row now opens a 458×524 window
with family search, family/face lists, custom entry, preview and OK/Cancel.
Installed metadata loads on a worker and returns through a weak UI handle.
Choosing a family/face edits the draft; only OK or custom-entry Enter sends font
intent. Windows keyboard checks open the chooser, edit an uncommitted custom
name and activate Cancel; the window closes with sans-serif, size 55, pause and
volume 37 unchanged. Scoped evidence: tests/visual/font-picker-cancel-windows-evidence.json.
Release build, strict lint and unit checks pass (the catalog test runs in both
the app and probe targets). This is provisional UI, not pixel parity. Search,
face selection, OK commit and glyph preview still need GUI checks. Native owner/
modality, Escape cancellation, localized ordering and other OS runtime remain
open. All changes remain local.

Local installed-font discovery: an owned metadata catalog now groups families,
localized aliases and faces through pinned fontdb 0.23.0. Duplicate source
entries collapse; internal/empty names are filtered and search covers Unicode
aliases without case sensitivity. A read-only Windows probe reports 72 families,
166 faces and 84 aliases in 19 ms, with self-name searches matching every family.
Counts are host-specific and do not prove complete platform font enumeration.
Only counts are recorded in tests/visual/font-catalog-windows-evidence.json;
no user font names, paths or bytes are saved. The font chooser UI and worker
integration, localized display ordering, previews, engine resolution/fallback
and macOS/Linux runtime remain open. Strict lint, formatting and all 33 unit
tests pass; the focused catalog test covers grouping, duplicates, hidden names
and Unicode alias searching. No public update was made.

Local subtitle font contract: shared font-name intent and optional native
observation now preserve Unicode and custom family strings. Empty custom entry
restores the pinned sans-serif default. Control characters are rejected before
native dispatch; names are single FFI arguments, including spaces and commas.
Native tests with an external SRT selected change serif, monospace and a custom
Unicode name, then restore the default while preserving size, border, colors,
delay, selection and pause. All 15 native tests pass in 10.32 seconds, all 32
unit tests and strict lint pass. This observes the configured option, not actual
installed-font selection or glyph rendering. Font chooser UI, family/face lists,
search/preview, persistence and other OS runtime remain open. Inventory now has
98 atomic requirements and is incomplete. Changes remain local.

Local color editor focus boundary: unhandled popup keys are now consumed within
the editor while channel arrows remain active and Tab navigates channels.
Windows checks at paused position 2 show p and Shift+Page Down do not pause or
seek, Left edits red 255 → 254 and Right restores 255, Tab reaches green, and
Escape restores the text well's focus. Pause is also preserved when pressing p
immediately after opening the popup before channel focus. Evidence and hashes:
tests/visual/subtitle-color-focus-windows-evidence.json. Release build, strict
lint and all 30 unit tests pass. Native panel appearance, rendered subtitles
and other OS runtime remain open. Nothing was published.

Local subtitle color UI: shared Scale/Font/Color/Border/Shadow rows now include
observed color wells and continuous RGB/opacity editing. Unknown observations
disable their well. Windows keyboard checks open the text editor, change red
255 → 64 and opacity 255 → 128, then reopen to observe those native values;
Escape restores well focus. Text is restored to opaque white. Border remains
opaque black and shadow remains clear; pause and volume 37 are preserved.
Scoped evidence: tests/visual/subtitle-color-wells-windows-evidence.json.
Release build, strict lint and all 30 unit tests pass. No tracks were loaded in
this UI check, so it does not verify rendered colors. The shared RGBA popup is
an interim control, not full IINA color panel parity. Native panel appearance,
icons/metrics, mouse routing, border/shadow GUI edits, preference persistence
and other OS runtime remain open. All changes stay local.

Local subtitle color backend: portable typed RGBA commands now control text,
border and shadow colors independently. Native observations parse RGB/ARGB hex
and three/four-channel values; invalid or unavailable values remain unknown.
Startup explicitly sets IINA's opaque white text, opaque black border and clear
shadow defaults. Tests with an external SRT selected verify alpha-preserving
updates and restoration while retaining font size 55, border width 3, subtitle
timing and selection, and pause. All 14 native tests pass in 9.93 seconds;
30 unit tests and strict lint pass. Inventory now contains 97 atomic rows and
remains incomplete. Color picker UI, preference persistence, rendered styling
and other OS runtime remain open. All changes stay local.

Local ten-minute seek verification now uses original CC0 long-silence.wav
(1300 seconds). Native paused seeks verify 100 → 700 → 100 seconds within 0.1 s,
preserving volume 37 with no error. Windows Shift+Page Up/Down from the focused
timeline likewise observe 100 → 700 → 99.99996, retaining pause and volume;
an additional Control modifier is rejected. Scoped evidence and hashes:
tests/visual/long-seek-interior-windows-evidence.json. All 13 native tests and
28 unit tests pass, strict lint passes, and all ten fixture hashes validate.
Video keyframe accuracy and macOS/Linux runtime verification remain open.
No public update was made.

Local long-seek shortcuts: shared controls now route unmodified-primary
Shift+Page Up/Down to relative +600/-600 seconds, matching the pinned default
input configuration. Windows keyboard checks on catalog.mkv move from paused
end to zero and then to the approximate forward-seek endpoint at 4.5 seconds,
preserving pause and volume 37. The five-second fixture proves keyboard routing
and endpoint behavior, not the exact ten-minute displacement; long-media and
other OS runtime checks remain open. Evidence:
tests/visual/long-seek-shortcuts-windows-evidence.json. Release build, strict lint,
formatting and all 28 unit tests pass. Inventory now has 94 atomic rows and
remains incomplete. Nothing was published.

Local subtitle picker fix verified on Windows: the rebuilt e36015b player now
steps font size 55 → 60 → 55 and outline width 3 → 4 → 3 using the keyboard.
The observed cursor follows both native values. Pause, volume 37, delay 0,
position 100 and neutral scale remain unchanged; the player closes cleanly.
Scoped accessibility evidence and binary hash are saved in
tests/visual/subtitle-picker-index-windows-evidence.json. No subtitle tracks were
loaded, so this verifies control routing rather than rendered subtitle styling.
Mouse popup selection, screenshots and other OS runtime checks remain open.
All work remains local under the milestone publication policy.

Local subtitle style Windows test and picker fix: accessibility confirms Scale,
Font and Border order and the observed defaults. Scale reduction and keyboard
reset route correctly while paused. Font keyboard Down exposed a real bug: the
picker displayed 55 but its cursor started at the first list item, selecting 30.
A shared ObservedSizePicker now synchronizes both current index and display text
from native observations for font and outline size, including external changes.
The old binary's evidence is saved in subtitle-style-controls-windows-evidence.json;
the subsequent runtime retest is recorded above. Strict lint and all 28 unit tests pass.
Visual capture still fails twice with CreateForMonitor error 0x80070057 after
fresh target selection; rendered layout and mouse popup routing remain pending.
Nothing was published.

Local subtitle style row layout and traceability: implemented style controls now
follow Scale, Font and Border order with inline labels, a 30px scale row and
60px size pickers derived from the pinned SubStyleView. Shared geometry remains
provisional: font chooser, color/shadow wells, source icons, exact spacing/theme
and rendered parity are still missing. Five atomic inventory rows now distinguish
style eligibility, scale/reset, font size, outline width and numeric delay entry,
with scoped evidence and explicit remaining differences. All 92 inventory rows
pass traceability validation; this is not a completeness or parity percentage.
Strict lint and all 28 unit tests pass. Runtime row layout and keyboard traversal
after rearrangement remain pending. Changes remain local.

Local subtitle outline width increment: the shared style picker now offers the
pinned border values zero, 0.25, 0.5, one, 1.5, two, 2.5, three, four and five.
A typed finite nonnegative setter and optional native outline observation back
the control; startup pins the reference default three. Unknown values disable
editing, and bitmap-only subtitle selection hides it with other text controls.
Real SRT playback verifies default three and widths zero, 0.25, five and three
while retaining font size 55, position 100, timing and pause without errors.
All twelve native tests pass (9.83 seconds), with 28 unit tests, strict lint and
formatting passing. Rendered outlines, picker routing, source-matched scale/font/
border layout, preferences and other-OS runtime remain pending. Work stays local.

Local subtitle font-size increment: typed finite positive font size dispatch and
optional native observation now back the shared style picker. Choices 25–75 in
steps of five follow the pinned SubStyleView; startup explicitly pins size 55
from Preference.subTextSize. Unknown size disables editing and bitmap-only
selection hides the text controls. Real SRT playback verifies observed default
55 and sizes 25, 75 and 55 with pause, position and delay retained. All twelve
native tests pass (9.83 seconds); the added default assertion passes a focused
native run (0.50 seconds). All 28 unit tests passed before the explicit startup
option; strict lint and formatting pass on the final code. Picker keyboard/
pointer routing, popup appearance, rendered font size, preference persistence
and other-OS runtime remain pending. No public update was made.

Local portable subtitle edit validation: numeric text parsing and typed Primary/
Secondary lane targeting now live in riina-core rather than the application UI
callback. The shared contract preserves double precision beyond f32, accepts
blank/whitespace reset and finite offsets outside the slider range, and rejects
nonfinite, overflowing and malformed input before any command is created. Two
new tests cover both lanes, including 1.0000000000000002 precision and invalid
numeric strings. All 28 unit tests, strict lint and formatting pass. Locale-aware
decimal separators and reference display formatting remain pending; comma input
is explicitly rejected by the current invariant parser. No OS-specific services
were introduced and no public update was made.

Local numeric delay focus completion: successful text submission now returns
focus to the player's shortcut scope, matching the pinned subtitle editor's
content-view focus handoff. Invalid or nonfinite numeric text stays editable
and reports an error; nonfinite values are rejected before queueing rather than
waiting for the decoder worker to reject them. Queue rejection also retains
field focus. Strict lint, formatting and all 26 unit tests pass for the focus
callback change; the subsequent finite-value guard passes lint and formatting.
Runtime focus handoff and invalid-entry correction remain unverified. Playback
renderer and cross-platform ownership are unchanged. Changes remain local.

Local numeric delay refresh Windows verification: the fixed release displays
observed primary delay one after slider input, then displays zero after clearing
and submitting the numeric field. Secondary keyboard text -7.375 submits to its
native delay without resuming playback. Switching to Primary restores zero in
both field and slider; switching back restores secondary -7.375 in both. Six
observations with binary/source hashes are recorded in numeric-delay-refresh-
windows-evidence.json. The release build passes and the player closes cleanly.
This verifies the edited-field binding fix and secondary numeric routing at
paused EOF without subtitle tracks, not rendered synchronization or visual parity.
Locale formatting and focus-return parity remain pending. Changes remain local.

Local numeric delay Windows verification and binding fix: actual keyboard typing
6.125 leaves native delay zero until Enter, then the observed slider reports
6.125 while playback remains paused. Clearing and submitting the field resets
native delay to zero. The same test exposed an edited TextInput retaining a blank
draft instead of displaying the observed zero. Explicit value/lane change handlers
now restore observed text after editing breaks the declarative binding. The old
binary's scoped evidence is saved in numeric-delay-windows-evidence.json; this
binding fix still needs runtime verification. Strict lint and all 26 unit tests
pass. Secondary numeric routing, formatter/focus parity and rendered geometry
remain pending. Changes stay local.

Local numeric subtitle delay entry: the shared lane editor now has a custom
keyboard-editable seconds field alongside its slider. Enter submits native
double-precision text rather than constraining values to ±5 seconds; blank input
resets the chosen lane to zero, and invalid text reports a validation error.
Observed delay remains authoritative. Real two-lane playback accepts primary
6.125 seconds and secondary -7.375 seconds together while preserving selection
and pause, then restores primary timing. All 26 unit tests pass. GUI text editing,
blank/reset routing, locale formatting, focus return and rendered field geometry
remain unverified; the reference formatter's significant-digit presentation is
still pending. This increment remains local.

Local ASS style eligibility native verification: an original CC0 ASS fixture
with two cues is reproducibly generated and hashed without bundling a font.
Real playback decodes its first cue and observes the ASS warning state. Selecting
plain SRT removes the warning; selecting ASS in secondary restores it while
preserving the primary SRT ID and pause. Disabling primary retains the secondary
ASS warning; disabling secondary removes it while retaining available text
controls. All twelve native tests pass (9.73 seconds), with strict lint,
formatting and all fixture hashes verified. Rendered ASS styling/warnings, bitmap
runtime fixtures and other-OS runtime remain pending. Changes remain local.

Local subtitle style eligibility increment: the portable model now combines both
selected subtitle lanes to determine text-control availability and ASS warnings.
The pinned codec rules classify PGS and DVB as bitmap; absent or unknown codecs
retain text controls. With no selected subtitles text controls remain available.
Only bitmap-selected lanes hide text scaling; a text/bitmap mix keeps it visible.
A selected ASS lane shows the reference warning, while unselected subtitle and
audio tracks cannot trigger it. Seven modeled catalog cases pass in a new unit
test; all 26 unit tests, strict lint and formatting pass. Runtime bitmap/ASS
fixtures, warning rendering and other-OS behavior remain unverified. Font/color/
border controls are still missing. Nothing was published.

Local subtitle text scale increment: the shared subtitle pane now includes a
text-size slider and reset action. The reference -5 to +5 slider uses rounded
0.05 magnitude increments with a reciprocal reduction below its neutral point;
reset maps to native scale one. Values display observed native scale and editing
is disabled until scale is known. An absolute typed scale command accepts finite
native values 0–100. Real SRT playback verifies absolute scales 1/6, six and one
while retaining pause, position, delay and the selected external subtitle without
native errors. All eleven native tests pass (9.76 seconds), with 25 unit tests,
strict lint and formatting passing. GUI mapping/reset, rendered text size,
bitmap/ASS style eligibility warnings, light theme and other-OS runtime remain
pending. Work remains local.

Local relocated subtitle picker verification: reverse Tab traversal from the
delay slider reaches secondary segment, primary segment and Load Subtitle.
Enter opens the native picker; Escape cancels and restores Load Subtitle focus.
An attempted fixture import remains unverified: the helper's refreshed tree
exposes filename edit element 127, but two set-value attempts report that element
unavailable in cached app state; Alt+n leaves focus in the search field. No file
was imported. Scoped evidence is saved in subtitle-load-picker-windows-evidence.json.
The player closes cleanly. This is a picker automation limitation, not evidence
of a product import failure; native external-subtitle tests remain separate.
No public update was made.

Local subtitle lane editor Windows verification: keyboard Enter and Space switch
the shared editor without resuming playback. Primary delay one second survives
secondary delay minus one second and position 25; primary position 60 then leaves
both secondary settings intact on switching back. Native observations displayed
by the sliders prove callback routing independently of editor selection. Section
order is present in the accessibility tree. Source/binary hashes and nine scoped
observations are stored in subtitle-lane-editor-windows-evidence.json. Playback
was paused at EOF and no subtitle tracks were loaded, so actual subtitle rendering,
track selection, scrolling and accessible checked-state exposure remain pending.
The test player closed cleanly. Nothing was published.

Local subtitle lane selector refinement: the editor's generic large text buttons
are replaced by a compact equal-width segmented control with 12px labels and
explicit accessible checked state. Choosing a segment is idempotent; keyboard
Enter/Space and pointer activation share the same lane-selection callback while
modified playback shortcuts bubble. Focus has a visible indicator. Strict lint,
formatting and all 25 unit tests pass. Native segment appearance, light theme,
runtime keyboard switching and exact macOS dimensions remain unverified; this
uses original portable UI geometry and does not claim pixel-perfect parity.
Changes remain local.

Local subtitle section ordering increment: Primary and Secondary track lists now
precede Load Subtitle and the shared timing/position editor, matching the pinned
SidebarSubtitlesPane section sequence. All subtitle sections share the scrolling
surface so growing track catalogs do not reserve fixed space outside it. Audio
loading and delay retain their existing placement. Keyboard handlers remain on
the relocated controls. Strict lint, formatting and all 25 unit tests pass;
runtime scrolling/focus traversal after relocation and rendered geometry remain
pending. Style sections, warnings and exact container spacing are still missing.
This is structural parity progress, not a pixel-perfect claim. Work stays local.

Local primary position native verification: the two-lane subtitle integration
test now applies primary positions 40, zero, 150 and 100 after secondary edits.
Observed primary position changes preserve secondary position 100, both delays,
both selected lane IDs, pause and an error-free native state. All eleven native
tests pass (9.68 seconds), with formatting and strict lint passing. This closes
the new absolute setter's native verification gap; actual rendered placement,
GUI routing and other-OS runtime remain unverified. Nothing was published.

Local subtitle editor consolidation: separate primary/secondary timing controls
are replaced with one shared delay/position section and a Primary/Secondary lane
selector, defaulting to Primary as in the pinned SubPositionDelayView. Switching
the editor changes only which observed values and command targets it exposes;
track selection is independent. Primary position now has an absolute typed setter
with the same finite 0–150 native validation as secondary; both UI sliders retain
the reference 0–100 integer range. A new core test verifies both property targets,
accepted boundaries and invalid values. Strict lint, formatting, all 25 unit tests
and all eleven native tests pass (9.68 seconds). Native regression tests do not
exercise the new primary absolute setter; GUI lane switching/routing, segment
geometry, section ordering, numeric entry and reset parity remain pending.
Changes remain local.

Local secondary subtitle position UI increment: the shared pane now exposes the
native secondary position through a keyboard-accessible slider. Its 0–100 range
and integer dispatch follow the pinned SubPositionDelayView; the engine's wider
0–150 range remains available through the existing typed command. Unknown native
position disables editing. Strict lint, formatting and all 24 unit tests pass.
Runtime GUI routing, rendered placement and the reference's shared segmented
primary/secondary position-delay layout remain pending. While auditing reset,
SidebarSubtitlesPane.swift SubDelayView.resetButtonAction was found to dispatch
audio delay zero; observable macOS reset behavior must be established before
implementing that action. This increment remains local.

Local secondary subtitle delay UI increment: the shared subtitle pane now exposes
a separate -5 to +5 second secondary delay slider with 0.1 second keyboard steps.
It dispatches the typed secondary command and displays native observations; an
unknown delay disables editing instead of presenting an assumed known zero.
The pinned SidebarSubtitlesPane defines the same slider range. All 24 unit tests,
strict lint and formatting pass, and all eleven native tests pass (9.62 seconds),
including independent shifted primary/secondary cues and reset. GUI routing and
rendered layout remain unverified; reference tick snapping, numeric entry, reset
button and section ordering remain incomplete. Nothing was published.

Local subtitle loading accessibility increment: the shared external audio/subtitle
load button now accepts keyboard focus, Enter and Space without consuming modified
player shortcuts. Windows release runtime testing with catalog.mkv confirmed Tab
reaches Load Subtitle, Enter opens its native picker and Escape cancels it and
restores focus to the load button. Further Tab navigation reaches Subtitle delay,
primary visibility, primary None, secondary visibility and secondary None in order.
Strict lint, formatting, release build and all 24 unit tests pass. This verifies
focus traversal and picker cancellation, not actual track activation or rendered
parity; external audio and other-OS runtime remain pending. Work stays local.

Local catalog keyboard increment: audio, primary subtitle, secondary subtitle,
chapter and playlist rows now expose keyboard focus and an Enter activation path.
Mouse selection retains the existing single/double-click behavior. Modified keys
and Space bubble to the player shortcuts rather than changing a focused track.
Secondary rows remain disabled without loaded media. Strict lint, formatting and
all 24 workspace unit tests pass; Slint compilation validates the focus handlers.
Runtime focus traversal, activation and rendered focus geometry remain unverified.
No public GitHub update was made.

Local secondary selector UI increment: the shared subtitle pane now exposes
Primary and Secondary track lists and independent visibility controls, following
the pinned pane's two-lane structure. Secondary rows retain native lane metadata
and stable ID tokens; None disables only the secondary lane. Unknown visibility
disables its control until a native observation arrives. Strict lint and all
24 workspace unit tests pass. GUI selection/routing and rendered parity remain
pending; load/timing section ordering and switch geometry are still provisional.
Work remains local.

Local secondary positioning increment: typed native secondary position accepts
finite values from zero to 150 and observes an optional validated position.
The two-lane SRT test verifies positions 25, zero, 150 and 100 while retaining
primary position, primary delay/selection, secondary delay and pause without
native errors. All eleven native tests pass (10.19 seconds), with 24 unit tests
and strict lint. Positioning UI, rendered geometry and other-OS runtime remain
pending; work remains local.

Local secondary timing increment: typed absolute secondary delay and optional
native observation are implemented. A real two-lane SRT test applies one second
of secondary delay and observes the primary second cue alongside the secondary
first cue at 2.3 seconds, preserving primary delay and selection. Reset restores
zero delay and aligned first cues. All eleven native tests pass (10.20 seconds),
with 24 unit tests and strict lint. Secondary timing controls, rendered output
and other-OS runtime remain pending. Work remains local.

Local secondary-subtitle increment: typed selection, independent visibility and
secondary text observation are implemented, with Alt+v connected in shared UI.
Track catalogs retain native lane metadata so secondary-only decoding cannot
highlight a primary selection. Native verification observes distinct primary/
secondary text together, independent visibility and disabling either selector,
retained pause and cleared secondary text on replacement. A distinct original
CC0 SRT fixture is generated and hashed. Secondary ASS override now defaults to
scale as in pinned IINA rather than mpv's strip default. The secondary selector
UI, GUI Alt+v routing, visual styling and other-OS runtime remain pending.
All eleven Windows native tests pass (9.43 seconds), with 24 unit tests, strict
lint, formatting and the release build. All original/new fixture hashes match.
Work remains local.

Local subtitle layout increment: G/F adjust native scale by +/-0.1, r raises
subtitle position by one point, and R/t lower it by one point, matching the
pinned default input config. Optional observed scale/position retain unknown
values as unknown. Ten Windows native tests pass (9.46 seconds), including
queued scale 1 → 2 → 1 and position 100 → 85 → 100, native range clamping and
retained pause/track/delay. The larger shortcut tree exposed a build-script
stack overflow; Slint compilation now uses a joined portable worker with an
explicit eight-MiB stack. GUI routing, actual rendered layout and other-OS
runtime remain pending. All 24 unit tests, strict lint, formatting and the
Windows release build pass after the compiler fix. Work remains local.

Local track-shortcut increment: native cycling now covers video, audio and
primary subtitles, retaining the engine's disabled sentinel and external-track
ordering. Shared routing adds macOS Command+Control+A/S/V and Windows/Linux
Control+Alt+A/S/V, Shift+primary+A/S panel toggles and plain v subtitle visibility.
Ten Windows native tests pass (9.34 seconds), including internal/external audio
cycling through disabled, external subtitle disable/restore and video disable/
restore with pause and unrelated selections retained. All 24 unit tests, strict
lint and the release build pass. GUI routing, other layouts/AltGr, visual feedback
and other-OS runtime remain pending. This increment stays local.

Local file-repeat increment: Shift+Command+L (macOS) / Shift+Control+L
(Windows/Linux) toggles only current-file repeat. Native integration verifies
actual EOF-to-start playback wrapping, file-repeat priority when both modes
are enabled, retention of playlist repeat when file repeat is removed, and
file-only on/off toggling without resuming pause. All nine Windows native tests
pass (9.28 seconds), with 24 unit tests, strict lint and the release build.
Windows keyboard/accessibility observations verify Off → File → Off with
timeline focus and unchanged paused position. Rendered feedback, repeat OSD,
other keyboard layouts and other-OS runtime remain pending. Evidence is in
tests/visual/file-repeat-windows-evidence.json. No public update was made.

Local chapter increment: Shift+primary+> / < invoke native relative chapter
navigation; Shift+primary+C / P toggle the chapter and playlist panels. Primary
is Command on macOS and Control on Windows/Linux. Eight Windows native tests
pass (9.40 seconds), verifying forward/backward chapter seeks, pause retention
and backward navigation at the first chapter, alongside 24 unit tests, strict
lint and the release build. Actual Windows keyboard/accessibility evidence
verifies navigation with timeline focus and both panel toggles. Last-chapter
transitions, backward restart threshold, rendered parity and other-OS runtime
remain open. See tests/visual/chapter-shortcut-windows-evidence.json. Work stays
local; no public GitHub update was made.

Local volume/speed increment: shortcut deltas and speed multipliers now operate
on native current values rather than a coalesced UI snapshot. Native volume
starts capped at 100; enabling boost permits 200, and disabling it clamps the
current value without overwriting queued changes with a stale display value.
Eight Windows native tests pass (9.19 seconds), including queued coarse volume
changes, both caps, boost removal, repeated speed multiplication and reset while
paused. All 24 unit tests, strict lint and the release build pass. Shared Rust
and Slint paths preserve platform boundaries. This increment has not been GUI
tested or run on macOS/Linux and remains local.

Local audio timing increment: Shift+( and Shift+) adjust audio delay by
+/-0.5 seconds; Alt adds fine +/-0.1-second steps, and Shift+_ resets it.
Decoder-relative commands preserve rapid adjustments. All eight explicit
Windows native tests pass (8.98 seconds), alongside 24 unit tests, strict lint
and the release build. Actual Windows keyboard/accessibility observations
verify audio and uppercase subtitle timing steps and reset with slider focus,
retaining pause at two seconds. Evidence is in
tests/visual/timing-shortcut-windows-evidence.json. Screen capture still fails;
rendered feedback, acoustic synchronization and other-OS runtime remain open.
No public GitHub update was made.

Local subtitle timing increment: uppercase Z/X adjust delay by -/+0.5 seconds,
Alt+Z/X use -/+0.1 seconds, and C resets it. Relative commands accumulate on
the decoder rather than reusing a stale UI snapshot. Eight Windows native
tests pass (8.85 seconds), including queued fine/coarse bursts and reset while
paused; all 24 unit tests and strict lint pass. Keyboard routing and other-OS
runtime verification remain pending. This work stays local.

Local subtitle increment: shared Shift+Right/Left shortcuts invoke native cue
navigation. Observed-clock/text tests verify next/previous cues with retained
pause and unchanged delay, then repeat with a 0.25-second delay and restart the
current cue. All eight explicit Windows native tests pass (8.75 seconds), with
24 workspace unit tests and strict lint. GUI routing, embedded-subtitle prefetch,
secondary cue selection, boundaries and rendering/style parity remain open.

Local Windows GUI evidence now verifies focused-slider Alt+Right/Left and
period/comma frame navigation, fine volume increments and Control+period Stop.
The seek clock also appears in accessibility observations. Capture still fails
with the monitor helper's 0x80070057, so rendered appearance and exact OSD timing
remain unverified. See tests/visual/frame-shortcut-windows-evidence.json.

Local shortcut increment: backward frame stepping now accompanies forward
stepping. Alt+arrows and comma/period invoke the frame actions; primary-modifier
arrows navigate the playlist, and primary-period stops. Modified arrows bubble
out of sliders. Default pause/mute/speed shortcuts and fine volume increments
are connected; Enter enters fullscreen without toggling it off. Seven native
tests pass, including observed one-frame forward/backward movement on the
24-fps fixture while paused. GUI input routing, boundary/VFR behavior and
other-OS runtime verification remain open. No public update was made.

Local A–B lifecycle increment: the backend now resets A/B options on new media,
matching the pinned reference configuration, rather than only clearing cached
markers. Native coverage verifies replacement, Stop/reopen and playlist advance
by playing past the previous B point. Stop/failure clears cached points and
end-file invalidates pending seek completion. UI parity and other-OS runtime
validation remain open; this increment has not been pushed publicly.

Local playback increment: typed A–B point setting and native point cycling are
connected to shared playback, with observed optional numeric points. Command–L
on macOS and Control–L on Windows/Linux invoke the default action. Five Windows
native integration tests pass (2.56 seconds), including an actual wrap from B
back to A, point cycling and clearing. Point editing preserves the pinned minimum
nonzero timestamp for zero. Menus, timeline markers, A–B OSD, persistence rules
and other-OS runtime comparison remain incomplete. Incremental work stays local;
public GitHub updates are reserved for substantial verified milestones.

Shared preferences now retain the OSD enable flag and timeout, defaulting to true
and one second for existing settings. Timeout validation rejects values the
event-loop timer cannot represent. Windows strict lint and 23 unit tests pass;
matching settings controls and OSD types beyond seeking remain incomplete.

Reference run 38007160214 succeeded operationally but its images are rejected
for matching-state comparison: early capture shows OSD over black video, while
the settled capture is white. Both IPC brackets remain paused at two seconds.
Rendered media content must be verified independently from the engine clock.

Real macOS arm64 libmpv integration now passes all four tests (run 38005017036,
3.45 seconds) with Homebrew mpv 0.41.0 on macOS 26.6.2. See
tests/visual/macos-native-evidence.json for the exact runtime hash and commit.
Full application builds and the subsequent concurrency/failure storage tests
also pass on macOS/Linux in run 38004680216. These tests use null video output;
displayed video, dialogs, audible output and HDR presentation remain unverified.
Official IINA 1.5.0 reference run 38005949498 now provides a reviewed paused
seek state at 2 seconds. It establishes the seek clock/progress panel, with the
main controls hidden; matched-size riina comparison remains pending.

The shared application now displays a transient seek clock/progress panel from
observed libmpv SEEK/PLAYBACK_RESTART events. A retained completion counter
survives coalesced snapshots; failed command submission does not fabricate a
clock update. Replacement/Stop hides the panel. Geometry, colors and the 2.5-second
timeout were provisional. The timeout now uses the pinned IINA default of one
second; rendering, fade and actual event-relative timing remain unverified.
Windows strict lint, 22 unit tests and four real native integration tests pass;
the paused-seek test now requires a native completion notification as well as
the observed 2-second clock. GUI rendering and port runtime validation are open.
The shared OSD reducer now defers coalesced completion notifications until the
clock is usable and no newer seek is in flight. Tests cover replacement, Stop,
failure suppression, rapid completions and clock ticks that must not extend the
display deadline. Native integration results above are from the preceding
backend change; the reducer-only increment passes workspace tests and lint.

Full macOS/Linux application lint, workspace unit tests and linked development
builds now pass in run 38004581488 at a48a1de. Evidence is recorded in
tests/visual/application-port-ci-evidence.json. Subsequent storage tests are
still running on macOS; native decoding run 38005017036 has been dispatched.
No GUI runtime, HDR presentation, packaging or exact interface parity is implied.

macOS file discovery now delegates filename ordering to Foundation's documented
localizedStandardCompare, using retained strings and a worker autorelease pool.
The dedicated Unicode/numeric worker test awaits macOS application CI. Windows
checks/lint pass locally; Linux retains its provisional numeric ordering.

2026-10-10 application port increment: macOS/Linux preference, library discovery
and asynchronous owned picker backends now connect to the shared GL player.
Dialog intent is shared; Windows retains its existing COM implementation.
A new application CI matrix lints/tests/links on macOS/Linux. Results and native
runtime behavior are pending; these systems are not yet supported targets.
See ADR 0004 for remaining collation, modality, packaging and HDR gaps.

2026-10-10 portability increment: folder discovery, extension filtering,
exact-path deduplication and playlist/disc bypass rules now live in riina-core.
The application supplies its native filename comparator through a small adapter.
Four filesystem/selection tests now join the three-OS portable CI matrix;
Windows retains a separate native filename-order test. Local formatting, strict
workspace lint and all 18 unit tests pass. Portable checks now contain 17 tests;
full macOS/Linux application builds and playback remain open near-term gates.

Milestone 0 foundation is implemented and built. Initial inventory is intentionally
incomplete: 64 atomic parity rows and 19 plugin surfaces are tracked. Milestone 1
is in progress; real SDR playback works, but HDR architecture, DPI coverage and
full accessibility/visual fidelity gates are not closed.

Published source: https://github.com/deminden/riina. At the user's request,
original riina contributions now use MIT and an explicit independent-development
policy (docs/licensing.md). Slint uses Royalty-free 2.0 with the official badge
in the public README; mpv ABI notices remain ISC. Third-party references and
native binaries retain their own terms. This source grant does not certify
clean-room provenance or close the native redistribution audit.

The separate D3D11 experiment now runs native gpu-next video with an owned Slint
overlay. Runtime logs report a 16-bit float scRGB swap chain on the observed SDR
desktop. Mouse/keyboard controls, Unicode input, fullscreen, maximize and shutdown
were exercised. This is presentation feasibility, not verified end-to-end HDR;
see ADR 0002 and `tests/visual/d3d11-probe-evidence.json`.

Original PQ/HLG 10-bit fixtures now verify effective BT.2020 metadata and
replacement/stop behavior. After the user enabled Windows HDR, the native probe
queried a 10-bit PQ/BT.2020 desktop and reported a rgba16hf/scRGB target with
1499-nit engine peak. PQ/HLG/SDR presentation and keyboard overlay controls were
exercised; actual captures/metadata are in `tests/visual/hdr-lifecycle-evidence.json`.
This is an observed HDR route, not calibrated color accuracy or full UI parity.
Undefined native target ratios no longer cause playback errors.

`riina --native-video <media>` now uses this candidate behind the same player
controls and sidebars as the default renderer. The shared controller is in
player_ui.rs; native_window.rs handles HWND ownership, client sizing and focus.
Actual native GUI checks covered PQ/HLG, fullscreen/Escape, toolbar Tab/Enter,
Stop, Ctrl+O after Stop, SDR recreation and popup-focused shutdown at 300% scale.
Initial focus/layout failures were corrected and rechecked. The default GL route
also passed playback and keyboard chapter-sidebar checks. Current evidence:
tests/visual/native-shared-evidence.json. Video blur and full window/DPI coverage
remain open; native output is experimental and opt-in.

At the user's request, the local toolchain and CI pin are now Rust 1.99.0, the
current stable release, using edition 2024. Crates inherit rust-version and deny
unsafe operations without explicit unsafe blocks. Current idioms include let
chains for state/ownership checks. Slint/libmpv remain independently pinned.

File pickers now retain their native owner, serialize requests and protect the
native video host while the Shell dialog is open. Cancellation and dispatch
failure restore modality without recursively scheduling a dropped guard.
Formatting, strict Clippy, 12 unit tests and the optimized Windows build pass.
Interactive testing caught and fixed event-thread-only handle acquisition and
cross-thread owner disabling. Default GL cancellation/reopening and final native
HDR cancellation/shutdown now have evidence in
tests/visual/dialog-lifecycle-evidence.json. File acceptance and external-track
dialog routes remain unverified because the automation helper cannot reliably
address the owned transient controls. Earlier native captures retain their own
binary hashes.

Open File now allows multiple selections and submits one validated OpenFiles
intent: replace with the first item, append the rest, and resume playback. A real
libmpv test verifies paused-playlist replacement, retained order and advancing to
HLG video (0.52 seconds). GUI acceptance remains unverified. IINA's file filtering,
deduplication/natural sorting, folder expansion and separate-player preferences
are still required; this implements its current-player batch branch only.
The full four-test native suite passes (2.10 seconds), alongside all 12 unit
tests, strict Clippy and the optimized Windows build. Historical accessibility
text captures mark their branding redaction; their image captures remain historical.

Dialog media/playlist selections now apply the pinned blacklist, exact-path
deduplication and natural filename sorting within lexically ordered parent paths.
A single media playlist file bypasses the blacklist for mpv expansion. Fourteen
unit tests, strict Clippy and the optimized build pass. Explorer comparison is a
Windows adaptation; locale equivalence, aliases and directory expansion remain
open, along with GUI acceptance and separate-player preferences.

Reference: IINA v1.5.0, **fbcff4df7183b335880396af0655f6bb89d46442**.
Acquisition and native binary hashes: reference/upstream-lock.json and native-lock.json.
No existing repository code was present. Git was initialized without committing.

Environment: native Windows 11 Home x64, version 10.0.26300; NVIDIA GeForce RTX
4070 Ti, driver 32.0.16.1656. Rust 1.99.0 MSVC (edition 2024), Slint/slint-build 1.13.1. Build
tools were downloaded/extracted under tools only; no system-wide installation,
security changes, default-app registration or administrator steps.

Implemented: three Cargo workspace crates; typed commands and state; validated
preference parsing; isolated dynamically loaded mpv ABI; event worker/property
observation; bounded intent queue and coalesced UI snapshots; asynchronous command
IDs; EOF/load-error handling; GL render context ownership and GPU FBO surface;
Slint custom OSC and grayscale sliders; open-file dialog and first CLI media argument;
play/pause/stop/relative and exact seek/volume/mute/speed/frame-step; observed timeline;
fullscreen/resize wiring; atomic local volume persistence; synthetic audiovisual fixture;
reference fetch, native dependency fetch, build helper and traceability checker.

Added event-owned mpv node decoding for track, chapter and playlist catalogs; typed
command plans; Audio/Subtitles/Chapters/Playlist sidebars. Playlist supports multi-file
append, single selection, remove, up/down reordering, clear except current entry,
repeat cycling and current-entry reload. Stable mpv IDs protect queued UI selection.
External subtitle loading/reload, primary visibility/delay and external audio/delay
are implemented. Chapter activation seeks to observed time and resumes playback.
Twelve unit tests and three explicit native integration tests pass. GUI verified chapter
activation, manual UTF-8 subtitle loading/rendering, accessible delay, multi-file
append and reordering. Final layout/disabled accessibility states were captured after
source width/side defaults and delay spacing were corrected. Appearance remains provisional; secondary subtitles, multi-row
selection, clipboard, shuffle/sort, numeric delay input and full styling are pending.

Artifacts:

- `target/x86_64-pc-windows-msvc/release/riina.exe`: optimized unsigned development executable.
- `target/debug/riina.exe`: earlier debug executable; rebuild before relying on its latest state.
- `tools/mpv/libmpv-2.dll`: pinned development runtime, loaded from the workspace or beside the executable.
- `tests/visual/`: actual Windows captures and capture manifest; not upstream references.

Build/run:

```powershell
./scripts/dev.ps1 -CargoArguments @('build','--release','--locked','--target','x86_64-pc-windows-msvc')
& ./target/x86_64-pc-windows-msvc/release/riina.exe ./tests/fixtures/moving-square.avi
```

The wrapper selects workspace-local Rust and import libraries when normal MSVC tools
are unavailable. In an ordinary MSVC developer shell use equivalent direct Cargo commands.
Native binary redistribution is not yet ready; don't move the EXE alone and expect playback.

Use top-right sidebar buttons to open Audio/Subtitles settings (trailing) or
Chapters/Playlist (leading). Double-click chapter/playlist entries to play them.
Add Files opens a multi-selection native picker. Remove/Clear affect playlist references,
not the files on disk. Original catalog.mkv, manual-captions.srt and external-tone.wav
fixtures are generated locally and hash-checked by scripts/check-fixtures.py.

CI configuration is in `.github/workflows/windows.yml`: pinned checkout action,
Rust toolchain, formatting/lint/domain tests, traceability and actual Windows application
compilation. The two initial Windows CI runs completed successfully on GitHub. Hosted compilation is not Windows 11 GUI
or native playback evidence; those checks remain explicitly separate.

See verification.md for exact commands/results and GUI limitations. No live macOS
reference, visual difference scores, calibrated HDR accuracy, bitstream audio, ARM64 or performance
thresholds were tested. Audio output was observed through libmpv; no independent
auditory listening test was performed. No claims of exact portable parity are made.

Known gaps: initial requirement/menu/preference inventory; source-matched light/compact/
docked layouts; actual sampled-video blur and licensed vector icons; full playlist/chapter/
track/subtitle workflows; multiple windows/history/resume; plugins/WebView2; OCR;
localization; drag/drop; network fixtures and helpers; Windows media/power/shell
integration; structured diagnostics; codec/native license audit and portable packaging;
HDR presentation, DPI matrix and stress/performance baselines. The command queue reports
overload rather than dropping consequential intents; seek-specific coalescing remains pending.

Fresh check after the user's HDR confirmation again detected DISPLAY1 as 10-bit
PQ/BT.2020 and presented the PQ fixture through rgba16hf/scRGB with the owned
controls visible. Logs: tools/hdr-current.stderr and tools/d3d11-probe.log.
These logs are local development artifacts; screenshots do not measure HDR luminance.

Next concrete implementation step: close native blur/composition, narrow-window,
modal ownership and mixed-monitor DPI gaps, then choose the production direction
in ADR 0001. Continue source-matched control geometry/icons and observable behavior inventory.

Recursive folder startup now discovers and orders media on a background worker in
both playback routes. Sixteen unit tests pass; both routes have observed folder
playlist/pause/shutdown evidence. GUI folder selection, scan cancellation/resource
limits and link/permission edge cases remain open. See folder-opening-evidence.json.

The project is now riina. Original state/commands/preferences stay in riina-core,
which has no Windows or GUI dependency. Windows dialogs, window ownership,
preferences storage, library resolution and Shell comparison live under
crates/app/src/platform/windows. The Windows dependency is target-scoped.
Shared Slint/controller behavior remains above these native services. Other OS
backends are near-term work, not implemented or tested. Volume boost is opt-in and
defaults off; disabling it restores the 100% cap.

GUI volume extension verified on the default renderer at 300% DPI: enabled
200%, disable/clamp to 100%, disabled-range enforcement and persisted off state.
Dark control contrast was corrected and visually inspected. See
tests/visual/riina-volume-evidence.json. Exact IINA visual parity remains open.

The shared floating panel now uses the recorded 460 x 67 outer geometry; actual
GL/native captures confirm these bounds and seeking input. Full material/icon/
typography parity still needs matching reference captures. The portable core
workflow tests and lint passed on Windows/macOS/Linux (run 38002087280).
Full application ports still require native services and actual playback evidence.

Original MIT vector controls now replace font-dependent playback and main
sidebar glyphs. GL/native rendering and native mute-state changes were observed
at 300% DPI; exact reference shapes remain unverified. The asset manifest now
contains 13 checked author/license/hash records. The portable matrix is expanded
to mpv interface unit tests, and HWND startup is gated to Windows.

Expanded portable CI passes 13 core/mpv-interface tests and strict lint on
Windows/macOS/Linux (run 38002664122). Native engine loading, GUI and application
ports are excluded from this evidence. See portable-interface-ci-evidence.json.

The shared controller no longer accepts a Windows window identifier. Native
startup/handle validation and diagnostics live in the presentation backend;
UiSession retains its target until the player shuts down. Native PQ, seeking,
fullscreen/Escape and close plus GL startup/seeking/close passed at 300% DPI.
Native logs use %LOCALAPPDATA%/riina/native-player.log. See ADR 0003 and
presentation-boundary-evidence.json. Other OS presentations remain open.
