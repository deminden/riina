# Status · 2026-10-09

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
