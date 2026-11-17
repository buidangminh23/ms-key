import AppKit
import ApplicationServices
import Carbon
import Foundation

public struct RunningWindowInfo: Identifiable, Hashable, Sendable {
    public let id: String
    public let processName: String
    public let windowTitle: String
    public let bundleIdentifier: String

    public init(processName: String, windowTitle: String, bundleIdentifier: String) {
        self.processName = processName
        self.windowTitle = windowTitle
        self.bundleIdentifier = bundleIdentifier
        self.id = "\(processName.lowercased())\u{001F}\(windowTitle.lowercased())\u{001F}\(bundleIdentifier)"
    }
}

public enum MacAppDetector {
    public static func frontmostAppAndWindowTitle() -> (appName: String, windowTitle: String) {
        if AXIsProcessTrusted() {
            let sys = AXUIElementCreateSystemWide()
            var focusedRef: CFTypeRef?
            if AXUIElementCopyAttributeValue(sys, kAXFocusedApplicationAttribute as CFString, &focusedRef) == .success,
               let axAppRef = focusedRef {
                let axApp = axAppRef as! AXUIElement
                var pid: pid_t = 0
                if AXUIElementGetPid(axApp, &pid) == .success,
                   let running = NSRunningApplication(processIdentifier: pid) {
                    return (canonicalName(for: running), focusedWindowTitle(of: axApp))
                }
            }
        }
        if let app = NSWorkspace.shared.frontmostApplication {
            let title = AXIsProcessTrusted()
                ? focusedWindowTitle(of: AXUIElementCreateApplication(app.processIdentifier))
                : ""
            return (canonicalName(for: app), title)
        }
        return ("", "")
    }

    public static func frontmostAppIdentifier() -> String {
        frontmostAppAndWindowTitle().appName
    }

    public static func focusedWindowTitle(of axApp: AXUIElement) -> String {
        guard AXIsProcessTrusted() else { return "" }
        var windowRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(axApp, kAXFocusedWindowAttribute as CFString, &windowRef) == .success,
              let win = windowRef else {
            return ""
        }
        let axWindow = win as! AXUIElement
        var titleRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(axWindow, kAXTitleAttribute as CFString, &titleRef) == .success,
              let title = titleRef as? String else {
            return ""
        }
        return title
    }

    public static func listRunningWindows() -> [RunningWindowInfo] {
        let selfBundle = Bundle.main.bundleIdentifier
        let axTrusted = AXIsProcessTrusted()
        var seen = Set<String>()
        var results: [RunningWindowInfo] = []
        for app in NSWorkspace.shared.runningApplications {
            guard app.activationPolicy == .regular else { continue }
            if let selfBundle, app.bundleIdentifier == selfBundle { continue }
            let proc = canonicalName(for: app).trimmingCharacters(in: .whitespacesAndNewlines)
            guard !proc.isEmpty else { continue }
            let title = axTrusted
                ? focusedWindowTitle(of: AXUIElementCreateApplication(app.processIdentifier)).trimmingCharacters(in: .whitespacesAndNewlines)
                : ""
            let dedupeKey = "\(proc.lowercased())\u{001F}\(title.lowercased())"
            if seen.insert(dedupeKey).inserted {
                results.append(RunningWindowInfo(
                    processName: proc,
                    windowTitle: title,
                    bundleIdentifier: app.bundleIdentifier ?? ""
                ))
            }
        }
        results.sort {
            let lhs = $0.processName.lowercased()
            let rhs = $1.processName.lowercased()
            if lhs != rhs { return lhs < rhs }
            return $0.windowTitle.lowercased() < $1.windowTitle.lowercased()
        }
        return results
    }

    public static func surroundingWordBeforeCaret(maxChars: Int = 32) -> String? {
        guard AXIsProcessTrusted() else { return nil }
        let sys = AXUIElementCreateSystemWide()
        var focusedRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(sys, kAXFocusedUIElementAttribute as CFString, &focusedRef) == .success,
              let focused = focusedRef else {
            return nil
        }
        let axEl = focused as! AXUIElement
        var rangeRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(axEl, kAXSelectedTextRangeAttribute as CFString, &rangeRef) == .success,
              let rangeVal = rangeRef,
              CFGetTypeID(rangeVal) == AXValueGetTypeID() else {
            return nil
        }
        var selectedRange = CFRange()
        guard AXValueGetValue(rangeVal as! AXValue, .cfRange, &selectedRange),
              selectedRange.length == 0,
              selectedRange.location > 0 else {
            return nil
        }
        let sliceStart = max(0, selectedRange.location - maxChars)
        let sliceLen = selectedRange.location - sliceStart
        guard sliceLen > 0 else { return nil }

        var queryRange = CFRange(location: sliceStart, length: sliceLen)
        var prefixText: String?
        if let paramRange = AXValueCreate(.cfRange, &queryRange) {
            var strRef: CFTypeRef?
            if AXUIElementCopyParameterizedAttributeValue(
                axEl,
                kAXStringForRangeParameterizedAttribute as CFString,
                paramRange,
                &strRef
            ) == .success, let s = strRef as? String {
                prefixText = s
            }
        }
        if prefixText == nil {
            var valRef: CFTypeRef?
            if AXUIElementCopyAttributeValue(axEl, kAXValueAttribute as CFString, &valRef) == .success,
               let full = valRef as? String {
                let ns = full as NSString
                if selectedRange.location <= ns.length {
                    prefixText = ns.substring(with: NSRange(location: sliceStart, length: sliceLen))
                }
            }
        }
        guard let prefix = prefixText, !prefix.isEmpty else { return nil }

        var reversedChars: [Character] = []
        for ch in prefix.reversed() {
            if ch.isLetter {
                reversedChars.append(ch)
            } else {
                break
            }
        }
        guard !reversedChars.isEmpty else { return nil }
        let word = String(reversedChars.reversed()).precomposedStringWithCanonicalMapping
        return word.isEmpty ? nil : word
    }

    public static func axCaretBoundsInCocoa() -> CGRect? {
        guard AXIsProcessTrusted() else { return nil }
        let sys = AXUIElementCreateSystemWide()
        var focusedRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(sys, kAXFocusedUIElementAttribute as CFString, &focusedRef) == .success,
              let focused = focusedRef else {
            return nil
        }
        let axEl = focused as! AXUIElement
        var rangeRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(axEl, kAXSelectedTextRangeAttribute as CFString, &rangeRef) == .success,
              let rangeVal = rangeRef,
              CFGetTypeID(rangeVal) == AXValueGetTypeID() else {
            return nil
        }
        var boundsRef: CFTypeRef?
        guard AXUIElementCopyParameterizedAttributeValue(
            axEl,
            kAXBoundsForRangeParameterizedAttribute as CFString,
            rangeVal,
            &boundsRef
        ) == .success,
              let boundsVal = boundsRef,
              CFGetTypeID(boundsVal) == AXValueGetTypeID() else {
            return nil
        }
        var axRect = CGRect.zero
        guard AXValueGetValue(boundsVal as! AXValue, .cgRect, &axRect),
              !axRect.isNull,
              !axRect.isInfinite,
              axRect.width > 0 || axRect.height > 0 else {
            return nil
        }
        let primaryHeight = NSScreen.screens.first?.frame.height ?? 0
        guard primaryHeight > 0 else { return nil }
        let cocoaRect = CGRect(
            x: axRect.origin.x,
            y: primaryHeight - axRect.origin.y - axRect.size.height,
            width: axRect.size.width,
            height: axRect.size.height
        )
        let onAnyScreen = NSScreen.screens.contains {
            $0.frame.insetBy(dx: -24, dy: -24).contains(cocoaRect.origin)
        }
        return onAnyScreen ? cocoaRect : nil
    }

    public static func anchorPointNearCaretOrCursor() -> NSPoint {
        if let caret = axCaretBoundsInCocoa() {
            return NSPoint(x: caret.maxX, y: caret.maxY)
        }
        return NSEvent.mouseLocation
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

    @MainActor
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
    case undoLastWord
    case convertSelection
}

public struct MacHotkeyTracker: Sendable {
    private var ctrlShiftPrimed = false
    private var ctrlWasDown = false
    private var shiftWasDown = false
    private var shiftPressAt: TimeInterval?
    private var lastShiftTapAt: TimeInterval?
    private var shiftTainted = false
    private var alphaShiftWasDown: Bool?
    public private(set) var holdEnglishActive = false

    public init() {}

    public mutating func reset() {
        ctrlShiftPrimed = false
        ctrlWasDown = false
        shiftWasDown = false
        shiftPressAt = nil
        lastShiftTapAt = nil
        shiftTainted = false
        alphaShiftWasDown = nil
        holdEnglishActive = false
    }

    public mutating func updateHoldEnglish(
        holdKey: HoldEnglishKey,
        flags: UInt64,
        keyCode: UInt16? = nil
    ) -> Bool? {
        let next = computeHoldEnglish(holdKey: holdKey, flags: flags, keyCode: keyCode)
        if next != holdEnglishActive {
            holdEnglishActive = next
            return next
        }
        return nil
    }

    private func computeHoldEnglish(holdKey: HoldEnglishKey, flags: UInt64, keyCode: UInt16?) -> Bool {
        switch holdKey {
        case .none:
            return false
        case .capsLock:
            return (flags & UInt64(CGEventFlags.maskAlphaShift.rawValue)) != 0
        case .rightShift:
            return resolveHoldModifier(
                flagOn: (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0,
                targetDeviceMask: 0x0004,
                otherDeviceMask: 0x0002,
                targetKeyCode: UInt16(kVK_RightShift),
                otherKeyCode: UInt16(kVK_Shift),
                flags: flags,
                keyCode: keyCode
            )
        case .leftShift:
            return resolveHoldModifier(
                flagOn: (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0,
                targetDeviceMask: 0x0002,
                otherDeviceMask: 0x0004,
                targetKeyCode: UInt16(kVK_Shift),
                otherKeyCode: UInt16(kVK_RightShift),
                flags: flags,
                keyCode: keyCode
            )
        case .rightAlt:
            return resolveHoldModifier(
                flagOn: (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0,
                targetDeviceMask: 0x0040,
                otherDeviceMask: 0x0020,
                targetKeyCode: UInt16(kVK_RightOption),
                otherKeyCode: UInt16(kVK_Option),
                flags: flags,
                keyCode: keyCode
            )
        case .rightCtrl:
            return resolveHoldModifier(
                flagOn: (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0,
                targetDeviceMask: 0x2000,
                otherDeviceMask: 0x0001,
                targetKeyCode: UInt16(kVK_RightControl),
                otherKeyCode: UInt16(kVK_Control),
                flags: flags,
                keyCode: keyCode
            )
        }
    }

    private func resolveHoldModifier(
        flagOn: Bool,
        targetDeviceMask: UInt64,
        otherDeviceMask: UInt64,
        targetKeyCode: UInt16,
        otherKeyCode: UInt16,
        flags: UInt64,
        keyCode: UInt16?
    ) -> Bool {
        guard flagOn else { return false }
        if (flags & targetDeviceMask) != 0 { return true }
        if (flags & otherDeviceMask) != 0 { return false }
        if let code = keyCode {
            if code == targetKeyCode { return true }
            if code == otherKeyCode { return false }
        }
        return holdEnglishActive
    }

    public mutating func handleFlagsChanged(
        preset: SwitchPresetKind,
        flags: UInt64,
        now: TimeInterval = ProcessInfo.processInfo.systemUptime,
        quickCapslockSwitch: Bool = false,
        keyCode: UInt16? = nil
    ) -> HotkeyEventSignal {
        let ctrl = (flags & UInt64(CGEventFlags.maskControl.rawValue)) != 0
        let shift = (flags & UInt64(CGEventFlags.maskShift.rawValue)) != 0
        let alt = (flags & UInt64(CGEventFlags.maskAlternate.rawValue)) != 0
        let cmd = (flags & UInt64(CGEventFlags.maskCommand.rawValue)) != 0
        let alphaShift = (flags & UInt64(CGEventFlags.maskAlphaShift.rawValue)) != 0

        let capsTriggered: Bool
        if quickCapslockSwitch {
            if keyCode == nil || keyCode == UInt16(kVK_CapsLock) {
                let prev = alphaShiftWasDown ?? !alphaShift
                alphaShiftWasDown = alphaShift
                capsTriggered = (alphaShift != prev) && !ctrl && !shift && !alt && !cmd
            } else {
                alphaShiftWasDown = alphaShift
                capsTriggered = false
            }
        } else {
            alphaShiftWasDown = alphaShift
            capsTriggered = false
        }

        defer {
            ctrlWasDown = ctrl
            shiftWasDown = shift
        }

        if capsTriggered {
            return .toggleVE
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
        flags: UInt64,
        undoHotkey: HotkeyComboModel? = nil,
        convertSelectionHotkey: HotkeyComboModel? = nil
    ) -> HotkeyEventSignal {
        ctrlShiftPrimed = false
        shiftPressAt = nil
        lastShiftTapAt = nil
        shiftTainted = true
        if matchesCombo(convertHotkey, vk: vk, flags: flags) {
            return .convertClipboard
        }
        if let undo = undoHotkey, matchesCombo(undo, vk: vk, flags: flags) {
            return .undoLastWord
        }
        if let convertSel = convertSelectionHotkey, matchesCombo(convertSel, vk: vk, flags: flags) {
            return .convertSelection
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
        guard combo.ctrl || combo.shift || combo.alt || combo.win_cmd || combo.key_code != 0 else {
            return false
        }
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

public struct AppRuleSession: Sendable {
    public private(set) var userPreferredVE: Bool
    public private(set) var activeApp: String = ""
    public private(set) var activeBehavior: AppBehaviorKind = .default

    public init(initialVE: Bool) {
        self.userPreferredVE = initialVE
    }

    public mutating func setUserVE(_ enabled: Bool) {
        userPreferredVE = enabled
    }

    public mutating func invalidateActive() {
        activeApp = ""
        activeBehavior = .default
    }

    public mutating func switchApp(_ appName: String, behavior: AppBehaviorKind) -> Bool? {
        let key = appName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !key.isEmpty else { return nil }
        if key == activeApp && behavior == activeBehavior {
            return nil
        }
        activeApp = key
        activeBehavior = behavior
        switch behavior {
        case .forceEnglish, .blacklist:
            return false
        case .forceVietnamese:
            return true
        case .default:
            return userPreferredVE
        }
    }
}
