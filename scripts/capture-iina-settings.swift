// Captures the pinned reference Settings window for visual research only.
import AppKit
import CoreGraphics
import Foundation
import ApplicationServices

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

func runAppleScript(_ source: String) throws -> String {
    let script = Process()
    script.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    script.arguments = ["-e", source]
    let output = Pipe()
    let errors = Pipe()
    script.standardOutput = output
    script.standardError = errors
    try script.run()
    script.waitUntilExit()
    let outputText = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
    guard script.terminationStatus == 0 else {
        let errorText = String(decoding: errors.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        throw NSError(domain: "RiinaReferenceCapture", code: Int(script.terminationStatus),
                      userInfo: [NSLocalizedDescriptionKey: "AppleScript could not control the pinned reference UI: \(errorText)"])
    }
    return outputText
}

func referenceWindows() -> [[String: Any]] {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.filter {
        ($0[kCGWindowOwnerName as String] as? String) == "IINA"
            && ($0[kCGWindowLayer as String] as? Int) == 0
    }
}

func settingsWindows() -> [[String: Any]] {
    let pageNames = Set(pages)
    return referenceWindows().filter { window in
        // AppKit titles this window after its selected page (for example,
        // "General"), not with the static "Settings" label.
        guard let name = window[kCGWindowName as String] as? String,
              pageNames.contains(name),
              let bounds = window[kCGWindowBounds as String] as? [String: Any],
              let width = bounds["Width"] as? NSNumber,
              let height = bounds["Height"] as? NSNumber else {
            return false
        }
        // The pinned source's 600x480 content window was observed as 608x500
        // including its titlebar; player and playlist windows are smaller or
        // use unrelated page names.
        return width.doubleValue >= 600 && height.doubleValue >= 480
    }
}

func axValue(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else {
        return nil
    }
    return value
}

func axString(_ element: AXUIElement, _ attribute: String) -> String? {
    axValue(element, attribute) as? String
}

func axChildren(_ element: AXUIElement) -> [AXUIElement] {
    axValue(element, kAXChildrenAttribute as String) as? [AXUIElement] ?? []
}

func axWindow(pid: Int, title: String) -> AXUIElement? {
    let application = AXUIElementCreateApplication(pid_t(pid))
    let windows = axValue(application, kAXWindowsAttribute as String) as? [AXUIElement] ?? []
    return windows.first { axString($0, kAXTitleAttribute as String) == title }
}

func axTree(_ element: AXUIElement, depth: Int = 0, budget: inout Int) -> [String: Any] {
    budget -= 1
    var node: [String: Any] = [
        "role": axString(element, kAXRoleAttribute as String) ?? "",
    ]
    let attributes: [(String, String)] = [
        ("title", kAXTitleAttribute as String),
        ("description", kAXDescriptionAttribute as String),
        ("value", kAXValueAttribute as String),
        ("identifier", "AXIdentifier"),
    ]
    for (key, attribute) in attributes {
        if let value = axString(element, attribute), !value.isEmpty {
            node[key] = value
        }
    }
    if depth < 10 && budget > 0 {
        let children = axChildren(element)
        if !children.isEmpty {
            var descendants = [[String: Any]]()
            for child in children {
                guard budget > 0 else { break }
                descendants.append(axTree(child, depth: depth + 1, budget: &budget))
            }
            node["children"] = descendants
        }
    }
    return node
}

func axContainsLabel(_ element: AXUIElement, _ page: String) -> Bool {
    let labels = [kAXTitleAttribute as String, kAXDescriptionAttribute as String,
                  kAXValueAttribute as String, "AXLabel"]
        .compactMap { axString(element, $0) }
    return labels.contains(page) || axChildren(element).contains { axContainsLabel($0, page) }
}

func axRow(_ element: AXUIElement, page: String) -> AXUIElement? {
    let role = axString(element, kAXRoleAttribute as String)
    if role == kAXRowRole as String && axContainsLabel(element, page) { return element }
    for child in axChildren(element) {
        if let row = axRow(child, page: page) { return row }
    }
    return nil
}

func selectSidebarPage(_ page: String, windowInfo: [String: Any]) -> String {
    guard AXIsProcessTrusted(),
          let pid = windowInfo[kCGWindowOwnerPID as String] as? Int,
          let currentTitle = windowInfo[kCGWindowName as String] as? String,
          let settingsWindow = axWindow(pid: pid, title: currentTitle) else {
        return "Accessibility API did not expose the active Settings window"
    }
    guard let row = axRow(settingsWindow, page: page) else {
        return "Accessibility API found no selectable \(page) row"
    }
    let pressed = AXUIElementPerformAction(row, kAXPressAction as CFString)
    if pressed == .success { return "AXPress succeeded" }
    let selected = AXUIElementSetAttributeValue(row, kAXSelectedAttribute as CFString, kCFBooleanTrue)
    return "AXPress=\(pressed.rawValue), AXSelected=\(selected.rawValue)"
}

do {
    guard permitted else {
        record["failure"] = "Screen-recording permission is unavailable; no permission request or change attempted"
        try saveRecord()
        exit(2)
    }

    // Use the app's visible Preferences menu item so CI follows the same action
    // and accessibility path as a person selecting it.
    do {
        try runAppleScript("tell application id \"com.colliderli.iina\" to activate")
        let menuItems = try runAppleScript("tell application \"System Events\" to tell process \"IINA\" to get name of every menu item of menu 1 of menu bar item \"IINA\" of menu bar 1")
        record["iina_menu_items"] = menuItems
        let label = menuItems.contains("Preferences") ? "Preferences" : "Settings"
        try runAppleScript("tell application \"System Events\" to tell process \"IINA\"\nset frontmost to true\nclick first menu item of menu 1 of menu bar item \"IINA\" of menu bar 1 whose name contains \"\(label)\"\nend tell")
    } catch {
        record["menu_automation_failure"] = error.localizedDescription
        // Command-comma is the same action's documented shortcut.
        _ = try runAppleScript("tell application id \"com.colliderli.iina\" to activate\ntell application \"System Events\" to tell process \"IINA\" to keystroke \",\" using {command down}")
    }

    let windowDeadline = Date().addingTimeInterval(12)
    var windows = settingsWindows()
    while windows.isEmpty && Date() < windowDeadline {
        Thread.sleep(forTimeInterval: 0.1)
        windows = settingsWindows()
    }
    record["observed_windows_after_open"] = referenceWindows()
    guard windows.count == 1, let window = windows.first,
          window[kCGWindowNumber as String] is UInt32 else {
        throw NSError(domain: "RiinaReferenceCapture", code: 3,
                      userInfo: [NSLocalizedDescriptionKey: "Expected exactly one visible Settings window"])
    }
    record["settings_window"] = window
    record["navigation_method"] = "Select the named source-list row through macOS Accessibility, then verify the window title before capture."
    record["accessibility_trusted"] = AXIsProcessTrusted()
    if let pid = window[kCGWindowOwnerPID as String] as? Int,
       let title = window[kCGWindowName as String] as? String,
       let accessibilityWindow = axWindow(pid: pid, title: title) {
        var budget = 1600
        record["settings_accessibility_tree"] = axTree(accessibilityWindow, budget: &budget)
        record["accessibility_nodes_recorded"] = 1600 - budget
    } else {
        record["settings_accessibility_tree"] = "No accessible Settings window"
    }
    try saveRecord()

    var captures = [[String: Any]]()
    for (index, page) in pages.enumerated() {
        if index > 0 {
            guard let previousWindow = settingsWindows().first(where: {
                ($0[kCGWindowName as String] as? String) == pages[index - 1]
            }) else {
                throw NSError(domain: "RiinaReferenceCapture", code: 9,
                              userInfo: [NSLocalizedDescriptionKey: "Could not find the selected \(pages[index - 1]) Settings window"])
            }
            let action = selectSidebarPage(page, windowInfo: previousWindow)
            record["last_accessibility_navigation"] = ["from": pages[index - 1], "to": page, "result": action]
            try saveRecord()
        }
        let pageDeadline = Date().addingTimeInterval(5)
        var currentWindow = settingsWindows().first { ($0[kCGWindowName as String] as? String) == page }
        while currentWindow == nil && Date() < pageDeadline {
            Thread.sleep(forTimeInterval: 0.1)
            currentWindow = settingsWindows().first { ($0[kCGWindowName as String] as? String) == page }
        }
        guard let currentWindow,
              let number = currentWindow[kCGWindowNumber as String] as? UInt32 else {
            throw NSError(domain: "RiinaReferenceCapture", code: 8,
                          userInfo: [NSLocalizedDescriptionKey: "Settings sidebar did not select \(page)"])
        }
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
        captures.append(["page": page, "image": imageName, "window": currentWindow,
                         "captured_at_utc": ISO8601DateFormatter().string(from: Date())])
        record["captures"] = captures
        try saveRecord()
    }
} catch {
    record["failure"] = error.localizedDescription
    try? saveRecord()
    fputs("Settings capture failed: \(error.localizedDescription)\n", stderr)
    exit(1)
}
