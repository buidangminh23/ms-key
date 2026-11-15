import AppKit
import ApplicationServices
import Carbon
import Foundation

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
    private var effectiveVE: Bool
    private var hotkeys = MacHotkeyTracker()
    private var session: SmartAppSessionMemory
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var tapRunLoop: CFRunLoop?
    private var tapThread: Thread?
    private var sandboxFocused = false
    private var pasteGeneration: UInt64 = 0
    private var pendingSavedPasteboard: [[(String, Data)]]?
    public var onSyncUI: (@Sendable (Bool, String, Bool) -> Void)?
    public var onConvertClipboard: (@Sendable () -> Void)?

    public init(rust: RustEngine, config: MSKeyConfig) {
        self.rust = rust
        self.config = config
        self.effectiveVE = config.vietnamese_enabled
        var mem = SmartAppSessionMemory(initialVE: config.vietnamese_enabled)
        mem.loadMemory(MacConfigStore.loadAppState())
        self.session = mem
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
        lock.lock()
        let alreadyRunning = eventTap != nil || tapThread != nil
        lock.unlock()
        if alreadyRunning { return true }
        guard Self.hasAccessibilityPermission(prompt: false) else { return false }

        let ready = DispatchSemaphore(value: 0)
        let thread = Thread { [weak self] in
            self?.runTapLoop(ready: ready)
        }
        thread.name = "mskey.event-tap"
        thread.qualityOfService = .userInteractive
        lock.lock()
        tapThread = thread
        lock.unlock()
        thread.start()
        _ = ready.wait(timeout: .now() + 2.0)

        lock.lock()
        let started = eventTap != nil
        if !started {
            tapThread = nil
        }
        lock.unlock()
        return started
    }

    private func runTapLoop(ready: DispatchSemaphore) {
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
            ready.signal()
            return
        }
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        let rl = CFRunLoopGetCurrent()
        lock.lock()
        eventTap = tap
        runLoopSource = source
        tapRunLoop = rl
        lock.unlock()

        CFRunLoopAddSource(rl, source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        ready.signal()
        CFRunLoopRun()

        CFRunLoopRemoveSource(rl, source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: false)
        CFMachPortInvalidate(tap)
        lock.lock()
        if let cur = eventTap, CFEqual(cur, tap) {
            eventTap = nil
            runLoopSource = nil
            tapRunLoop = nil
            tapThread = nil
        }
        lock.unlock()
    }

    public func stop() {
        lock.lock()
        let tap = eventTap
        let src = runLoopSource
        let rl = tapRunLoop
        eventTap = nil
        runLoopSource = nil
        tapRunLoop = nil
        tapThread = nil
        lock.unlock()
        if let tap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        if let rl {
            if let src {
                CFRunLoopRemoveSource(rl, src, .commonModes)
            }
            CFRunLoopStop(rl)
        }
    }

    public func updateConfig(_ cfg: MSKeyConfig) {
        lock.lock()
        config = cfg
        effectiveVE = cfg.vietnamese_enabled
        rust.setConfig(cfg)
        session.setUserVE(cfg.vietnamese_enabled, smartSwitch: cfg.smart_app_switch)
        let saveSmart = cfg.smart_app_switch
        let state = session.exportMemory()
        lock.unlock()
        if saveSmart {
            MacConfigStore.saveAppState(state)
        }
    }

    public func setSandboxFocused(_ focused: Bool) {
        lock.lock()
        sandboxFocused = focused
        if focused {
            rust.resetAll()
        }
        lock.unlock()
    }

    public func notifyAppActivated(_ appName: String) {
        lock.lock()
        let sync = applyAppSwitchInsideLock(appName)
        lock.unlock()
        if let sync {
            onSyncUI?(sync.0, sync.1, false)
        }
    }

    private func applyAppSwitchInsideLock(_ appName: String) -> (Bool, String)? {
        let smart = config.smart_app_switch
        let behavior = rust.behavior(for: appName)
        let switched = session.switchApp(appName, behavior: behavior, smartSwitch: smart)
        if let nextVE = switched {
            effectiveVE = nextVE
            rust.setEnabled(nextVE)
        }
        rust.resetAll()
        hotkeys.reset()
        return (effectiveVE, session.activeApp)
    }

    fileprivate func handleEvent(
        proxy: CGEventTapProxy,
        type: CGEventType,
        event: CGEvent
    ) -> Unmanaged<CGEvent>? {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            lock.lock()
            let tap = eventTap
            lock.unlock()
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
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
        let uiSync = sig == .toggleVE ? toggleVEInsideLock() : nil
        lock.unlock()
        if let uiSync {
            dispatchSyncUI(ve: uiSync.0, app: uiSync.1, beep: uiSync.2, userInitiated: true)
        }
        return Unmanaged.passUnretained(event)
    }

    private func handleKeyDown(_ event: CGEvent) -> Unmanaged<CGEvent>? {
        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let vk = MacKeycode.toPortableVK(keyCode)
        let flags = event.flags.rawValue
        let detectedApp = MacAppDetector.frontmostAppIdentifier()

        lock.lock()
        var pendingAppSync: (Bool, String)?
        if !detectedApp.isEmpty && detectedApp.lowercased() != session.activeApp {
            pendingAppSync = applyAppSwitchInsideLock(detectedApp)
        }
        let sig = hotkeys.handleKeyDown(
            preset: config.switch_preset,
            customSwitch: config.custom_switch_hotkey,
            convertHotkey: config.clipboard_convert_hotkey,
            vk: vk,
            flags: flags
        )
        if sig == .toggleVE {
            let uiSync = toggleVEInsideLock()
            lock.unlock()
            dispatchSyncUI(ve: uiSync.0, app: uiSync.1, beep: uiSync.2, userInitiated: true)
            return nil
        }
        if sig == .convertClipboard {
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            DispatchQueue.main.async { self.onConvertClipboard?() }
            return nil
        }

        let activeApp = session.activeApp
        let modMask = CGEventFlags.maskCommand.rawValue | CGEventFlags.maskControl.rawValue | CGEventFlags.maskAlternate.rawValue
        let hasAccelMod = (flags & modMask) != 0
        if sandboxFocused || hasAccelMod || (!activeApp.isEmpty && rust.behavior(for: activeApp) == .blacklist) {
            if hasAccelMod {
                rust.resetWord()
                pasteGeneration &+= 1
                pendingSavedPasteboard = nil
            }
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            return Unmanaged.passUnretained(event)
        }

        let disableNonUS = config.disable_on_non_us_layout
        let delayMs = config.send_delay_ms > 0
            ? min(Int(config.send_delay_ms), 25)
            : (config.mechanism == .compatibleHook ? 1 : 0)
        let opts = SendOptions(
            useClipboard: config.use_clipboard_send || config.mechanism == .compatibleHook,
            pasteShiftInsert: config.paste_via_shift_insert,
            standardMode: config.standard_key_send || MacAppDetector.isTerminalApp(activeApp),
            delayMs: delayMs,
            browserFix1: config.fix_browser_suggest_1 && MacAppDetector.isBrowserOrSpotlight(activeApp),
            browserFix2: config.fix_browser_suggest_2 && MacAppDetector.isBrowserOrSpotlight(activeApp)
        )
        lock.unlock()

        if let s = pendingAppSync {
            DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
        }
        if disableNonUS && !MacAppDetector.isUSKeyboardLayout() {
            return Unmanaged.passUnretained(event)
        }
        if MacAppDetector.isSecureInputOrPasswordFocused() {
            lock.lock()
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        return processKeyAction(event: event, keyCode: keyCode, opts: opts)
    }

    private func processKeyAction(event: CGEvent, keyCode: UInt16, opts: SendOptions) -> Unmanaged<CGEvent>? {
        lock.lock()
        let act: KeyActionResult
        if Int(keyCode) == kVK_Escape {
            act = rust.processEscape()
        } else if Int(keyCode) == kVK_Delete {
            act = rust.processBackspace()
        } else if MacKeycode.isNavigationKey(keyCode) {
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        } else if let scalar = extractScalar(from: event, keyCode: keyCode) {
            act = rust.processKey(scalar: scalar)
        } else {
            rust.resetWord()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        lock.unlock()
        return applyAction(act, original: event, triggerKeyCode: keyCode, opts: opts)
    }

    private func toggleVEInsideLock() -> (Bool, String, Bool) {
        let next = !effectiveVE
        effectiveVE = next
        config.vietnamese_enabled = next
        session.setUserVE(next, smartSwitch: config.smart_app_switch)
        rust.setEnabled(next)
        rust.resetAll()
        if config.smart_app_switch {
            MacConfigStore.saveAppState(session.exportMemory())
        }
        return (next, session.activeApp, config.beep_on_switch)
    }

    private func dispatchSyncUI(ve: Bool, app: String, beep: Bool, userInitiated: Bool) {
        DispatchQueue.main.async {
            if beep { NSSound.beep() }
            self.onSyncUI?(ve, app, userInitiated)
        }
    }

    private func extractScalar(from event: CGEvent, keyCode: UInt16) -> UInt32? {
        if Int(keyCode) == kVK_Return || Int(keyCode) == kVK_ANSI_KeypadEnter { return 10 }
        if Int(keyCode) == kVK_Tab { return 9 }
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
        triggerKeyCode: UInt16,
        opts: SendOptions
    ) -> Unmanaged<CGEvent>? {
        guard act.replace else {
            return Unmanaged.passUnretained(original)
        }
        var body = act.text
        var trailingVK: CGKeyCode?
        if (Int(triggerKeyCode) == kVK_Return || Int(triggerKeyCode) == kVK_ANSI_KeypadEnter), body.hasSuffix("\n") {
            body.removeLast()
            trailingVK = CGKeyCode(kVK_Return)
        } else if Int(triggerKeyCode) == kVK_Tab, body.hasSuffix("\t") {
            body.removeLast()
            trailingVK = CGKeyCode(kVK_Tab)
        }

        if act.backspaces > 0 {
            if opts.browserFix1 {
                CGEventEmitter.postUnicodeText("\u{200B}", singleChar: true, delayMs: opts.delayMs)
                CGEventEmitter.postBackspaces(1, delayMs: opts.delayMs)
            }
            if opts.browserFix2 {
                CGEventEmitter.postVirtualKey(CGKeyCode(kVK_RightArrow), flags: [], delayMs: opts.delayMs)
            }
            CGEventEmitter.postBackspaces(act.backspaces, delayMs: opts.delayMs)
        }
        if !body.isEmpty {
            if opts.useClipboard && body.count > 1 {
                emitViaPasteboard(body, pasteShiftInsert: opts.pasteShiftInsert, delayMs: opts.delayMs)
            } else {
                CGEventEmitter.postUnicodeText(body, singleChar: opts.standardMode, delayMs: opts.delayMs)
            }
        }
        if let trailingVK {
            if opts.useClipboard && body.count > 1 { usleep(8000) }
            CGEventEmitter.postVirtualKey(trailingVK, flags: [], delayMs: opts.delayMs)
        }
        return nil
    }

    private func emitViaPasteboard(_ text: String, pasteShiftInsert: Bool, delayMs: Int) {
        let result = runOnMainSync { () -> (UInt64, Int) in
            let pb = NSPasteboard.general
            self.lock.lock()
            let needSnapshot = self.pendingSavedPasteboard == nil
            self.pasteGeneration &+= 1
            let gen = self.pasteGeneration
            self.lock.unlock()

            let snapshot = needSnapshot ? PasteboardSnapshot.capture(pb) : nil
            if let snapshot {
                self.lock.lock()
                if self.pendingSavedPasteboard == nil {
                    self.pendingSavedPasteboard = snapshot
                }
                self.lock.unlock()
            }
            pb.clearContents()
            pb.setString(text, forType: .string)
            return (gen, pb.changeCount)
        }
        if delayMs > 0 {
            usleep(UInt32(min(delayMs, 10)) * 1000)
        }
        if pasteShiftInsert {
            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_Help), flags: .maskShift, delayMs: 0)
        } else {
            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_ANSI_V), flags: .maskCommand, delayMs: 0)
        }
        let gen = result.0
        let expectedCount = result.1
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(95 + delayMs)) {
            let pb = NSPasteboard.general
            self.lock.lock()
            guard self.pasteGeneration == gen else {
                self.lock.unlock()
                return
            }
            let saved = self.pendingSavedPasteboard
            self.pendingSavedPasteboard = nil
            self.lock.unlock()
            guard pb.changeCount == expectedCount, let saved else { return }
            PasteboardSnapshot.restore(saved, to: pb)
        }
    }

    private func runOnMainSync<T: Sendable>(_ work: @Sendable () -> T) -> T {
        if Thread.isMainThread {
            return work()
        }
        return DispatchQueue.main.sync(execute: work)
    }
}
