import AppKit
import ApplicationServices
import Carbon
import Foundation

public enum MacAppDetector {
    public static func frontmostAppIdentifier() -> String {
        if AXIsProcessTrusted() {
            let sys = AXUIElementCreateSystemWide()
            var focusedRef: CFTypeRef?
            if AXUIElementCopyAttributeValue(sys, kAXFocusedApplicationAttribute as CFString, &focusedRef) == .success,
               let axApp = focusedRef {
                var pid: pid_t = 0
                if AXUIElementGetPid(axApp as! AXUIElement, &pid) == .success,
                   let running = NSRunningApplication(processIdentifier: pid) {
                    return canonicalName(for: running)
                }
            }
        }
        if let app = NSWorkspace.shared.frontmostApplication {
            return canonicalName(for: app)
        }
        return ""
    }

    public static func canonicalName(for app: NSRunningApplication) -> String {
        if let url = app.executableURL?.lastPathComponent, !url.isEmpty {
            return url
        }
        if let name = app.localizedName, !name.isEmpty {
            return name
        }
        return app.bundleIdentifier ?? ""
    }

    public static func isBrowserOrSpotlight(_ appName: String) -> Bool {
        let lower = appName.lowercased()
        let targets = [
            "chrome", "safari", "firefox", "arc", "brave", "edge", "opera",
            "spotlight", "raycast", "alfred", "vivaldi", "orion",
        ]
        return targets.contains { lower.contains($0) }
    }

    public static func isUSKeyboardLayout() -> Bool {
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
              let idPtr = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else {
            return true
        }
        let sourceID = Unmanaged<CFString>.fromOpaque(idPtr).takeUnretainedValue() as String
        if sourceID.contains("keylayout.US")
            || sourceID.contains("keylayout.ABC")
            || sourceID.contains("keylayout.British") {
            return true
        }
        return !sourceID.contains("keylayout.")
    }
}

public enum HotkeyEventSignal: Equatable, Sendable {
    case none
    case toggleVE
    case convertClipboard
}

public struct MacHotkeyTracker: Sendable {
    private var ctrlShiftPrimed = false
    private var ctrlWasDown = false
    private var shiftWasDown = false

    public init() {}

    public mutating func reset() {
        ctrlShiftPrimed = false
        ctrlWasDown = false
        shiftWasDown = false
    }

    public mutating func handleFlagsChanged(preset: SwitchPresetKind, flags: UInt64) -> HotkeyEventSignal {
        let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
        let shift = (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0
        let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
        let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
        defer {
            ctrlWasDown = ctrl
            shiftWasDown = shift
        }
        guard preset == .ctrlShift else {
            ctrlShiftPrimed = false
            return .none
        }
        if alt || cmd {
            ctrlShiftPrimed = false
            return .none
        }
        if ctrl && shift {
            ctrlShiftPrimed = true
            return .none
        }
        if ctrlShiftPrimed && (ctrlWasDown || shiftWasDown) && !ctrl && !shift {
            ctrlShiftPrimed = false
            return .toggleVE
        }
        if (ctrl != ctrlWasDown || shift != shiftWasDown) && ctrlShiftPrimed && !(ctrl && shift) {
            ctrlShiftPrimed = false
            return .toggleVE
        }
        return .none
    }

    public mutating func handleKeyDown(
        preset: SwitchPresetKind,
        customSwitch: HotkeyComboModel,
        convertHotkey: HotkeyComboModel,
        vk: UInt32,
        flags: UInt64
    ) -> HotkeyEventSignal {
        ctrlShiftPrimed = false
        if matchesCombo(convertHotkey, vk: vk, flags: flags) {
            return .convertClipboard
        }
        switch preset {
        case .altZ:
            let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
            let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
            let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
            if alt && !ctrl && !cmd && vk == 0x5A {
                return .toggleVE
            }
        case .custom:
            if matchesCombo(customSwitch, vk: vk, flags: flags) {
                return .toggleVE
            }
        case .ctrlShift:
            break
        }
        return .none
    }

    private func matchesCombo(_ combo: HotkeyComboModel, vk: UInt32, flags: UInt64) -> Bool {
        let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
        let shift = (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0
        let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
        let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
        return ctrl == combo.ctrl
            && shift == combo.shift
            && alt == combo.alt
            && cmd == combo.win_cmd
            && vk == combo.key_code
    }
}

public struct SmartAppSessionMemory: Sendable {
    public private(set) var userPreferredVE: Bool
    public private(set) var activeApp: String = ""
    private var memory: [String: Bool] = [:]

    public init(initialVE: Bool) {
        self.userPreferredVE = initialVE
    }

    public mutating func setUserVE(_ enabled: Bool, smartSwitch: Bool) {
        userPreferredVE = enabled
        if smartSwitch && !activeApp.isEmpty {
            memory[activeApp.lowercased()] = enabled
        }
    }

    public mutating func switchApp(_ appName: String, behavior: AppBehaviorKind, smartSwitch: Bool) -> Bool? {
        let key = appName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !key.isEmpty, key != activeApp else {
            return nil
        }
        activeApp = key
        switch behavior {
        case .forceEnglish, .blacklist:
            return false
        case .forceVietnamese:
            return true
        case .default:
            if smartSwitch {
                return memory[key] ?? userPreferredVE
            }
            return userPreferredVE
        }
    }
}
