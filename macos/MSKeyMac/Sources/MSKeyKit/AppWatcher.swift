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

    public static func isTerminalApp(_ appName: String) -> Bool {
        let lower = appName.lowercased()
        let targets = [
            "terminal", "iterm", "warp", "alacritty", "kitty",
            "wezterm", "ghostty", "hyper", "tabby", "tmux",
        ]
        return targets.contains { lower.contains($0) }
    }

    public static func isSecureInputOrPasswordFocused() -> Bool {
        if IsSecureEventInputEnabled() {
            return true
        }
        guard AXIsProcessTrusted() else {
            return false
        }
        let sys = AXUIElementCreateSystemWide()
        var focusedRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(sys, kAXFocusedUIElementAttribute as CFString, &focusedRef) == .success,
              let focused = focusedRef else {
            return false
        }
        let axEl = focused as! AXUIElement
        var subroleRef: CFTypeRef?
        if AXUIElementCopyAttributeValue(axEl, kAXSubroleAttribute as CFString, &subroleRef) == .success,
           let subrole = subroleRef as? String,
           subrole == (kAXSecureTextFieldSubrole as String) {
            return true
        }
        var roleRef: CFTypeRef?
        if AXUIElementCopyAttributeValue(axEl, kAXRoleAttribute as CFString, &roleRef) == .success,
           let role = roleRef as? String,
           role == "AXSecureTextField" {
            return true
        }
        return false
    }

    public static func isUSKeyboardLayout() -> Bool {
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
              let idPtr = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else {
            return true
        }
        let sourceID = Unmanaged<CFString>.fromOpaque(idPtr).takeUnretainedValue() as String
        if sourceID.hasPrefix("com.apple.inputmethod.") {
            return false
        }
        let usLayouts = [
            "keylayout.US",
            "keylayout.ABC",
            "keylayout.British",
            "keylayout.Australian",
            "keylayout.Canadian",
            "keylayout.USExtended",
            "keylayout.USInternational-PC",
        ]
        if usLayouts.contains(where: { sourceID.contains($0) }) {
            return true
        }
        if sourceID.contains("keylayout.") {
            return false
        }
        return true
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
    private var shiftPressAt: TimeInterval?
    private var lastShiftTapAt: TimeInterval?
    private var shiftTainted = false

    public init() {}

    public mutating func reset() {
        ctrlShiftPrimed = false
        ctrlWasDown = false
        shiftWasDown = false
        shiftPressAt = nil
        lastShiftTapAt = nil
        shiftTainted = false
    }

    public mutating func handleFlagsChanged(
        preset: SwitchPresetKind,
        flags: UInt64,
        now: TimeInterval = ProcessInfo.processInfo.systemUptime
    ) -> HotkeyEventSignal {
        let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
        let shift = (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0
        let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
        let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
        defer {
            ctrlWasDown = ctrl
            shiftWasDown = shift
        }

        if preset == .ctrlShift {
            shiftPressAt = nil
            lastShiftTapAt = nil
            shiftTainted = false
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

        ctrlShiftPrimed = false
        guard preset == .shiftShift else {
            shiftPressAt = nil
            lastShiftTapAt = nil
            shiftTainted = false
            return .none
        }

        let otherMods = ctrl || alt || cmd
        if otherMods {
            shiftTainted = true
            shiftPressAt = nil
            lastShiftTapAt = nil
            return .none
        }

        if shift && !shiftWasDown {
            if shiftPressAt == nil {
                shiftPressAt = now
                shiftTainted = false
            }
        } else if !shift && shiftWasDown {
            let valid = !shiftTainted
                && (shiftPressAt.map { (now - $0) <= 0.35 } ?? false)
            shiftPressAt = nil
            shiftTainted = false
            if valid {
                if let prev = lastShiftTapAt, (now - prev) <= 0.40 {
                    lastShiftTapAt = nil
                    return .toggleVE
                }
                lastShiftTapAt = now
            } else {
                lastShiftTapAt = nil
            }
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
        shiftPressAt = nil
        lastShiftTapAt = nil
        shiftTainted = true
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
        case .ctrlSpace:
            let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
            let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
            let shift = (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0
            let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
            if ctrl && !alt && !shift && !cmd && vk == 0x20 {
                return .toggleVE
            }
        case .custom:
            if matchesCombo(customSwitch, vk: vk, flags: flags) {
                return .toggleVE
            }
        case .ctrlShift, .shiftShift:
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

    public mutating func loadMemory(_ saved: [String: Bool]) {
        for (k, v) in saved {
            let norm = k.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if !norm.isEmpty {
                memory[norm] = v
            }
        }
    }

    public func exportMemory() -> [String: Bool] {
        memory
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
