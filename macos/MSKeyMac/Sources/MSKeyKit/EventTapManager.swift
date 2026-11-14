import AppKit
import ApplicationServices
import Carbon
import Foundation

public enum MacKeycode {
    public static func toPortableVK(_ keyCode: UInt16) -> UInt32 {
        switch Int(keyCode) {
        case kVK_ANSI_A: return 0x41
        case kVK_ANSI_B: return 0x42
        case kVK_ANSI_C: return 0x43
        case kVK_ANSI_D: return 0x44
        case kVK_ANSI_E: return 0x45
        case kVK_ANSI_F: return 0x46
        case kVK_ANSI_G: return 0x47
        case kVK_ANSI_H: return 0x48
        case kVK_ANSI_I: return 0x49
        case kVK_ANSI_J: return 0x4A
        case kVK_ANSI_K: return 0x4B
        case kVK_ANSI_L: return 0x4C
        case kVK_ANSI_M: return 0x4D
        case kVK_ANSI_N: return 0x4E
        case kVK_ANSI_O: return 0x4F
        case kVK_ANSI_P: return 0x50
        case kVK_ANSI_Q: return 0x51
        case kVK_ANSI_R: return 0x52
        case kVK_ANSI_S: return 0x53
        case kVK_ANSI_T: return 0x54
        case kVK_ANSI_U: return 0x55
        case kVK_ANSI_V: return 0x56
        case kVK_ANSI_W: return 0x57
        case kVK_ANSI_X: return 0x58
        case kVK_ANSI_Y: return 0x59
        case kVK_ANSI_Z: return 0x5A
        case kVK_ANSI_0: return 0x30
        case kVK_ANSI_1: return 0x31
        case kVK_ANSI_2: return 0x32
        case kVK_ANSI_3: return 0x33
        case kVK_ANSI_4: return 0x34
        case kVK_ANSI_5: return 0x35
        case kVK_ANSI_6: return 0x36
        case kVK_ANSI_7: return 0x37
        case kVK_ANSI_8: return 0x38
        case kVK_ANSI_9: return 0x39
        case kVK_F1: return 0x70
        case kVK_F2: return 0x71
        case kVK_F3: return 0x72
        case kVK_F4: return 0x73
        case kVK_F5: return 0x74
        case kVK_F6: return 0x75
        case kVK_F7: return 0x76
        case kVK_F8: return 0x77
        case kVK_F9: return 0x78
        case kVK_F10: return 0x79
        case kVK_F11: return 0x7A
        case kVK_F12: return 0x7B
        case kVK_Space: return 0x20
        case kVK_Return, kVK_ANSI_KeypadEnter: return 0x0D
        case kVK_Tab: return 0x09
        case kVK_ANSI_Grave: return 0xC0
        default: return UInt32(keyCode)
        }
    }

    public static func isNavigationKey(_ keyCode: UInt16) -> Bool {
        switch Int(keyCode) {
        case kVK_LeftArrow, kVK_RightArrow, kVK_DownArrow, kVK_UpArrow,
             kVK_Home, kVK_End, kVK_PageUp, kVK_PageDown, kVK_ForwardDelete:
            return true
        default:
            return false
        }
    }
}

private let mskeyEventTag: Int64 = 0x4D53_4B59

private func cgEventTapCallback(
    proxy: CGEventTapProxy,
    type: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    let manager = Unmanaged<EventTapManager>.fromOpaque(userInfo).takeUnretainedValue()
    return manager.handleEvent(proxy: proxy, type: type, event: event)
}

public final class EventTapManager: @unchecked Sendable {
    private let lock = NSLock()
    private let rust: RustEngine
    private var config: MSKeyConfig
    private var hotkeys = MacHotkeyTracker()
    private var session: SmartAppSessionMemory
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    public var onSyncUI: (@Sendable (Bool, String) -> Void)?
    public var onConvertClipboard: (@Sendable () -> Void)?

    public init(rust: RustEngine, config: MSKeyConfig) {
        self.rust = rust
        self.config = config
        self.session = SmartAppSessionMemory(initialVE: config.vietnamese_enabled)
    }

    public static func hasAccessibilityPermission(prompt: Bool) -> Bool {
        if prompt {
            let key = "AXTrustedCheckOptionPrompt" as CFString
            let opts = [key: kCFBooleanTrue!] as CFDictionary
            return AXIsProcessTrustedWithOptions(opts)
        }
        return AXIsProcessTrusted()
    }

    @discardableResult
    public func start() -> Bool {
        if eventTap != nil { return true }
        guard Self.hasAccessibilityPermission(prompt: false) else { return false }
        let mask = (1 << CGEventType.keyDown.rawValue)
            | (1 << CGEventType.flagsChanged.rawValue)
            | (1 << CGEventType.leftMouseDown.rawValue)
            | (1 << CGEventType.rightMouseDown.rawValue)
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: CGEventMask(mask),
            callback: cgEventTapCallback,
            userInfo: refcon
        ) else {
            return false
        }
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        eventTap = tap
        runLoopSource = source
        return true
    }

    public func stop() {
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        if let src = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), src, .commonModes)
        }
        runLoopSource = nil
        eventTap = nil
    }

    public func updateConfig(_ cfg: MSKeyConfig) {
        lock.lock()
        config = cfg
        rust.setConfig(cfg)
        session.setUserVE(cfg.vietnamese_enabled, smartSwitch: cfg.smart_app_switch)
        lock.unlock()
    }

    public func notifyAppActivated(_ appName: String) {
        lock.lock()
        let smart = config.smart_app_switch
        let behavior = rust.behavior(for: appName)
        let switched = session.switchApp(appName, behavior: behavior, smartSwitch: smart)
        if let nextVE = switched {
            config.vietnamese_enabled = nextVE
            rust.setEnabled(nextVE)
        }
        rust.resetAll()
        hotkeys.reset()
        let ve = config.vietnamese_enabled
        let active = session.activeApp
        lock.unlock()
        onSyncUI?(ve, active)
    }

    fileprivate func handleEvent(
        proxy: CGEventTapProxy,
        type: CGEventType,
        event: CGEvent
    ) -> Unmanaged<CGEvent>? {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let tap = eventTap {
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            return Unmanaged.passUnretained(event)
        }
        if event.getIntegerValueField(.eventSourceUserData) == mskeyEventTag {
            return Unmanaged.passUnretained(event)
        }
        if type == .leftMouseDown || type == .rightMouseDown {
            lock.lock()
            rust.resetWord()
            hotkeys.reset()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        if type == .flagsChanged {
            return handleFlagsChanged(event)
        }
        if type == .keyDown {
            return handleKeyDown(event)
        }
        return Unmanaged.passUnretained(event)
    }

    private func handleFlagsChanged(_ event: CGEvent) -> Unmanaged<CGEvent>? {
        lock.lock()
        let sig = hotkeys.handleFlagsChanged(preset: config.switch_preset, flags: event.flags.rawValue)
        if sig == .toggleVE {
            toggleVEInsideLock()
        }
        lock.unlock()
        return Unmanaged.passUnretained(event)
    }

    private func handleKeyDown(_ event: CGEvent) -> Unmanaged<CGEvent>? {
        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let vk = MacKeycode.toPortableVK(keyCode)
        let flags = event.flags.rawValue

        lock.lock()
        let sig = hotkeys.handleKeyDown(
            preset: config.switch_preset,
            customSwitch: config.custom_switch_hotkey,
            convertHotkey: config.clipboard_convert_hotkey,
            vk: vk,
            flags: flags
        )
        switch sig {
        case .toggleVE:
            toggleVEInsideLock()
            lock.unlock()
            return nil
        case .convertClipboard:
            lock.unlock()
            onConvertClipboard?()
            return nil
        case .none:
            break
        }

        let activeApp = session.activeApp
        if !activeApp.isEmpty && rust.behavior(for: activeApp) == .blacklist {
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        let modMask = CGEventFlags.maskCommand.rawValue | CGEventFlags.maskControl.rawValue | CGEventFlags.maskAlternate.rawValue
        if (flags & modMask) != 0 {
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        if config.disable_on_non_us_layout && !MacAppDetector.isUSKeyboardLayout() {
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        let useClip = config.use_clipboard_send || config.mechanism == .compatibleHook
        let browserFix = config.fix_browser_suggest_1 && MacAppDetector.isBrowserOrSpotlight(activeApp)

        if Int(keyCode) == kVK_Escape {
            let act = rust.processEscape()
            lock.unlock()
            return applyAction(act, original: event, useClipboard: useClip, browserFix: browserFix)
        }
        if Int(keyCode) == kVK_Delete {
            let act = rust.processBackspace()
            lock.unlock()
            return applyAction(act, original: event, useClipboard: useClip, browserFix: browserFix)
        }
        if MacKeycode.isNavigationKey(keyCode) {
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        guard let scalar = extractScalar(from: event, keyCode: keyCode) else {
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        let act = rust.processKey(scalar: scalar)
        lock.unlock()
        return applyAction(act, original: event, useClipboard: useClip, browserFix: browserFix)
    }

    private func toggleVEInsideLock() {
        let next = !config.vietnamese_enabled
        config.vietnamese_enabled = next
        session.setUserVE(next, smartSwitch: config.smart_app_switch)
        rust.setEnabled(next)
        rust.resetAll()
        let beep = config.beep_on_switch
        let active = session.activeApp
        DispatchQueue.main.async {
            if beep { NSSound.beep() }
            self.onSyncUI?(next, active)
        }
    }

    private func extractScalar(from event: CGEvent, keyCode: UInt16) -> UInt32? {
        if Int(keyCode) == kVK_Return || Int(keyCode) == kVK_ANSI_KeypadEnter {
            return 10
        }
        if Int(keyCode) == kVK_Tab {
            return 9
        }
        var length = 0
        var chars = [UniChar](repeating: 0, count: 4)
        event.keyboardGetUnicodeString(maxStringLength: 4, actualStringLength: &length, unicodeString: &chars)
        guard length > 0 else { return nil }
        let s = String(utf16CodeUnits: Array(chars.prefix(length)), count: length)
        return s.unicodeScalars.first?.value
    }

    private func applyAction(
        _ act: KeyActionResult,
        original: CGEvent,
        useClipboard: Bool,
        browserFix: Bool
    ) -> Unmanaged<CGEvent>? {
        guard act.replace else {
            return Unmanaged.passUnretained(original)
        }
        if act.backspaces > 0 && browserFix {
            emitUnicodeText("\u{200B}")
            emitBackspaceCount(1)
        }
        if act.backspaces > 0 {
            emitBackspaceCount(act.backspaces)
        }
        if !act.text.isEmpty {
            if useClipboard && act.text.count > 1 {
                emitViaPasteboard(act.text)
            } else {
                emitUnicodeText(act.text)
            }
        }
        return nil
    }

    private func emitBackspaceCount(_ count: Int) {
        let src = CGEventSource(stateID: .privateState)
        for _ in 0..<count {
            if let down = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(kVK_Delete), keyDown: true),
               let up = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(kVK_Delete), keyDown: false) {
                down.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                up.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                down.post(tap: .cghidEventTap)
                up.post(tap: .cghidEventTap)
            }
        }
    }

    private func emitUnicodeText(_ text: String) {
        let src = CGEventSource(stateID: .privateState)
        let units = Array(text.utf16)
        var offset = 0
        while offset < units.count {
            let end = min(offset + 16, units.count)
            var slice = Array(units[offset..<end])
            if let down = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: true),
               let up = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: false) {
                down.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                up.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                down.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: &slice)
                up.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: &slice)
                down.post(tap: .cghidEventTap)
                up.post(tap: .cghidEventTap)
            }
            offset = end
        }
    }

    private func emitViaPasteboard(_ text: String) {
        DispatchQueue.main.async {
            let pb = NSPasteboard.general
            let prev = pb.string(forType: .string)
            pb.clearContents()
            pb.setString(text, forType: .string)
            let src = CGEventSource(stateID: .privateState)
            if let down = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: true),
               let up = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: false) {
                down.flags = .maskCommand
                up.flags = .maskCommand
                down.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                up.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
                down.post(tap: .cghidEventTap)
                up.post(tap: .cghidEventTap)
            }
            if let prev {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
                    pb.clearContents()
                    pb.setString(prev, forType: .string)
                }
            }
        }
    }
}
