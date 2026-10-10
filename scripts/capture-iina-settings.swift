// Captures the pinned reference Settings window for visual research only.
import AppKit
import CoreGraphics
import Foundation

let destination = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
let permitted = CGPreflightScreenCaptureAccess()
let pages = ["General", "UI", "Video", "Audio", "Subtitles", "Network", "Control", "Key Bindings", "Advanced", "Plugin", "Utilities"]
var record: [String: Any] = [
    "schema_version": 1,
    "captured_at_utc": ISO8601DateFormatter().string(from: Date()),
    "reference_version": "1.5.0",
    "screen_recording_preflight": permitted,
    "theme": UserDefaults.standard.string(forKey: "AppleInterfaceStyle") ?? "Light",
    "screens": NSScreen.screens.map { ["frame": NSStringFromRect($0.frame), "scale": $0.backingScaleFactor] as [String: Any] },
    "requested_pages": pages,
    "captures": [[String: Any]](),
    "visual_acceptance": "unreviewed; captures are research evidence, not accepted comparison goldens",
]

func saveRecord() throws {
    let data = try JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
    try data.write(to: destination.appendingPathComponent("settings-capture-metadata.json"), options: .atomic)
}

func runAppleScript(_ source: String) throws {
    let script = Process()
    script.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    script.arguments = ["-e", source]
    try script.run()
    script.waitUntilExit()
    guard script.terminationStatus == 0 else {
        throw NSError(domain: "RiinaReferenceCapture", code: Int(script.terminationStatus),
                      userInfo: [NSLocalizedDescriptionKey: "AppleScript could not control the pinned reference UI"])
    }
}

func settingsWindows() -> [[String: Any]] {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.filter {
        ($0[kCGWindowOwnerName as String] as? String) == "IINA"
            && ($0[kCGWindowName as String] as? String) == "Settings"
            && ($0[kCGWindowLayer as String] as? Int) == 0
    }
}

do {
    guard permitted else {
        record["failure"] = "Screen-recording permission is unavailable; no permission request or change attempted"
        try saveRecord()
        exit(2)
    }

    // Command-comma is IINA's Preferences shortcut; use its ordinary menu path.
    try runAppleScript("tell application id \"com.colliderli.iina\" to activate\ntell application \"System Events\" to tell process \"IINA\" to keystroke \",\" using {command down}")

    let windowDeadline = Date().addingTimeInterval(12)
    var windows = settingsWindows()
    while windows.isEmpty && Date() < windowDeadline {
        Thread.sleep(forTimeInterval: 0.1)
        windows = settingsWindows()
    }
    guard windows.count == 1, let window = windows.first,
          let number = window[kCGWindowNumber as String] as? UInt32 else {
        throw NSError(domain: "RiinaReferenceCapture", code: 3,
                      userInfo: [NSLocalizedDescriptionKey: "Expected exactly one visible Settings window"])
    }
    record["settings_window"] = window

    var captures = [[String: Any]]()
    var selectedPage = 0
    for (index, page) in pages.enumerated() {
        // The source list inserts a non-page subsection indicator after the
        // selected page, so later rows shift as navigation advances.
        let sidebarRow = index + 1 + (index > selectedPage ? 1 : 0)
        try runAppleScript("tell application \"System Events\" to tell process \"IINA\" to select row \(sidebarRow) of table 1 of window \"Settings\"")
        Thread.sleep(forTimeInterval: 0.35)

        let imageName = String(format: "iina-settings-%02d.png", index + 1)
        let imageURL = destination.appendingPathComponent(imageName)
        let capture = Process()
        capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        capture.arguments = ["-x", "-o", "-l", String(number), imageURL.path]
        try capture.run()
        capture.waitUntilExit()
        guard capture.terminationStatus == 0, FileManager.default.fileExists(atPath: imageURL.path) else {
            throw NSError(domain: "RiinaReferenceCapture", code: 4,
                          userInfo: [NSLocalizedDescriptionKey: "Could not capture Settings page \(page)"])
        }
        captures.append(["page": page, "image": imageName, "window": window,
                         "captured_at_utc": ISO8601DateFormatter().string(from: Date())])
        selectedPage = index
        record["captures"] = captures
        try saveRecord()
    }
} catch {
    record["failure"] = error.localizedDescription
    try? saveRecord()
    fputs("Settings capture failed: \(error.localizedDescription)\n", stderr)
    exit(1)
}
