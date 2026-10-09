// Reference-only capture tooling. No upstream application code or assets are reused.
import AppKit
import CoreGraphics
import Foundation

let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let fixture = CommandLine.arguments[2]
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
let permitted = CGPreflightScreenCaptureAccess()
func playerCandidates() -> [[String: Any]] {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.filter {
        ($0[kCGWindowOwnerName as String] as? String) == "IINA"
            && ($0[kCGWindowLayer as String] as? Int) == 0
    }
}
var candidates = playerCandidates()
var record: [String: Any] = [
    "schema_version": 1,
    "captured_at_utc": ISO8601DateFormatter().string(from: Date()),
    "screen_recording_preflight": permitted,
    "fixture": fixture,
    "theme": UserDefaults.standard.string(forKey: "AppleInterfaceStyle") ?? "Light",
    "screens": NSScreen.screens.map { screen in
        ["frame": NSStringFromRect(screen.frame), "scale": screen.backingScaleFactor] as [String: Any]
    },
    "candidate_windows": candidates,
    "reference_version": "1.5.0",
    "playback_request": ["paused": true, "position_seconds": 2],
    "visual_acceptance": "unreviewed; capture success alone is not a verified baseline",
]
func saveRecord() throws {
    let data = try JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
    try data.write(to: output.appendingPathComponent("capture-metadata.json"), options: .atomic)
}
guard permitted else {
    record["failure"] = "Screen-recording permission is unavailable; no permission request or change attempted"
    try saveRecord()
    exit(2)
}
func mediaWindows() -> [[String: Any]] {
    candidates.filter {
        ($0[kCGWindowName as String] as? String)?.contains(fixture) == true
    }
}
// Engine media readiness precedes the actual AppKit window. Probe this boundary
// before pausing startup, without replacing it with an assumed launch delay.
let windowDeadline = Date().addingTimeInterval(15)
while mediaWindows().isEmpty && Date() < windowDeadline {
    Thread.sleep(forTimeInterval: 0.1)
    candidates = playerCandidates()
}
record["candidate_windows"] = candidates
guard mediaWindows().count == 1, let window = mediaWindows().first,
      let number = window[kCGWindowNumber as String] as? UInt32 else {
    record["failure"] = "Expected exactly one visible IINA window named for the fixture"
    try saveRecord()
    exit(3)
}
record["selected_window"] = window
if CommandLine.arguments.dropFirst(3).contains("--wait-window") {
    record["capture_mode"] = "window-readiness only; no image captured"
    record.removeValue(forKey: "playback_request")
    try saveRecord()
    exit(0)
}
let destination = output.appendingPathComponent("iina-1.5.0-paused.png")
let capture = Process()
capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
capture.arguments = ["-x", "-o", "-l", String(number), destination.path]
try capture.run()
capture.waitUntilExit()
record["capture_exit_code"] = capture.terminationStatus
record["image"] = destination.lastPathComponent
try saveRecord()
guard capture.terminationStatus == 0, FileManager.default.fileExists(atPath: destination.path) else {
    exit(4)
}
