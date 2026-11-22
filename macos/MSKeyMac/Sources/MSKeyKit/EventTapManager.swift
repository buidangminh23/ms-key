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
    private var session: AppRuleSession
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var tapRunLoop: CFRunLoop?
    private var tapThread: Thread?
    private var tapGeneration: UInt64 = 0
    private var typingFocus: MacTypingFocus?
    private var requestedFocusPID: pid_t?
    var onFocusRefresh: (@Sendable () -> Void)?
    private var sandboxFocused = false
    private var bufferActive = false
    private var usesUSKeyboardLayout = true
    private var isSelectionPopupOpen = false
    private var pasteGeneration: UInt64 = 0
    private var pendingSavedPasteboard: [[(String, Data)]]?
    private let selectionQueue = DispatchQueue(label: "mskey.selection-convert")
    public var onSyncUI: (@Sendable (Bool, String, Bool) -> Void)?
    public var onConvertClipboard: (@Sendable () -> Void)?

    public init(rust: RustEngine, config: MSKeyConfig) {
        self.rust = rust
        self.config = config
        self.effectiveVE = config.vietnamese_enabled
        self.session = AppRuleSession(initialVE: config.vietnamese_enabled)
    }

    public static func hasAccessibilityPermission(prompt: Bool) -> Bool {
        if prompt {
            let key = "AXTrustedCheckOptionPrompt" as CFString
            let opts = [key: kCFBooleanTrue!] as CFDictionary
            return AXIsProcessTrustedWithOptions(opts)
        }
        return AXIsProcessTrusted()
    }

    public enum TapState: Equatable, Sendable {
        case stopped
        case starting
        case enabled
        case disabled
    }

    public enum HealthAction: Equatable, Sendable {
        case none
        case start
        case recover
        case stop
    }

    public static func healthAction(trusted: Bool, state: TapState) -> HealthAction {
        switch (trusted, state) {
        case (true, .stopped): return .start
        case (true, .disabled): return .recover
        case (false, .starting), (false, .enabled), (false, .disabled): return .stop
        default: return .none
        }
    }

    public var tapState: TapState {
        lock.lock()
        defer { lock.unlock() }
        guard let tap = eventTap else {
            return tapThread == nil ? .stopped : .starting
        }
        return CGEvent.tapIsEnabled(tap: tap) ? .enabled : .disabled
    }

    public func recover() {
        lock.lock()
        let tap = eventTap
        lock.unlock()
        if let tap {
            CGEvent.tapEnable(tap: tap, enable: true)
            if CGEvent.tapIsEnabled(tap: tap) { return }
        }
        stop()
        start()
    }

    @discardableResult
    public func start() -> Bool {
        lock.lock()
        let alreadyRunning = eventTap != nil || tapThread != nil
        lock.unlock()
        if alreadyRunning { return true }
        guard Self.hasAccessibilityPermission(prompt: false) else { return false }

        let ready = DispatchSemaphore(value: 0)
        lock.lock()
        tapGeneration &+= 1
        let generation = tapGeneration
        let thread = Thread { [weak self] in
            self?.runTapLoop(generation: generation, ready: ready)
        }
        thread.name = "mskey.event-tap"
        thread.qualityOfService = .userInteractive
        tapThread = thread
        lock.unlock()
        thread.start()
        _ = ready.wait(timeout: .now() + 2.0)

        lock.lock()
        let started = eventTap != nil
        lock.unlock()
        return started
    }

    private func runTapLoop(generation: UInt64, ready: DispatchSemaphore) {
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
            lock.lock()
            if tapGeneration == generation {
                tapThread = nil
            }
            lock.unlock()
            ready.signal()
            return
        }
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        let rl = CFRunLoopGetCurrent()
        lock.lock()
        guard tapGeneration == generation else {
            lock.unlock()
            CGEvent.tapEnable(tap: tap, enable: false)
            CFMachPortInvalidate(tap)
            ready.signal()
            return
        }
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
        tapGeneration &+= 1
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
        rust.setTemporaryEnglish(false)
        bufferActive = false
        hotkeys.reset()
        session.invalidateActive()
        session.setUserVE(cfg.vietnamese_enabled)
        lock.unlock()
    }

    func updateKeyboardLayout(isUS: Bool) {
        lock.lock()
        defer { lock.unlock() }
        guard usesUSKeyboardLayout != isUS else { return }
        usesUSKeyboardLayout = isUS
        if config.disable_on_non_us_layout {
            rust.resetAll()
            bufferActive = false
        }
    }

    public func setSandboxFocused(_ focused: Bool) {
        lock.lock()
        sandboxFocused = focused
        if focused {
            rust.resetAll()
            bufferActive = false
        }
        lock.unlock()
    }

    var hasWindowTitleRules: Bool {
        lock.lock()
        defer { lock.unlock() }
        return config.app_rules.contains { !$0.titlePattern.isEmpty }
    }

    func updateTypingFocus(_ focus: MacTypingFocus?, targetChanged: Bool) {
        lock.lock()
        typingFocus = focus
        if focus != nil { requestedFocusPID = nil }
        if targetChanged {
            rust.resetAll()
            rust.setTemporaryEnglish(false)
            hotkeys.reset()
            bufferActive = false
        }
        let sync = focus.flatMap { applyWindowCheckInsideLock(appName: $0.appName, windowTitle: $0.windowTitle) }
        lock.unlock()
        if let sync { onSyncUI?(sync.0, sync.1, false) }
    }

    public func notifyAppActivated(_ appName: String, windowTitle: String = "") {
        lock.lock()
        let sync = applyWindowCheckInsideLock(appName: appName, windowTitle: windowTitle)
        lock.unlock()
        if let sync {
            onSyncUI?(sync.0, sync.1, false)
        }
    }

    private func applyWindowCheckInsideLock(appName: String, windowTitle: String) -> (Bool, String)? {
        let normApp = appName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !normApp.isEmpty else { return nil }
        let behavior = rust.behaviorForWindow(processName: appName, windowTitle: windowTitle)
        let sameApp = (normApp == session.activeApp)
        if sameApp && behavior == session.activeBehavior {
            return nil
        }
        let switched = session.switchApp(appName, behavior: behavior)
        if let nextVE = switched {
            effectiveVE = nextVE
            rust.setEnabled(nextVE)
        }
        rust.resetAll()
        rust.setTemporaryEnglish(false)
        bufferActive = false
        if !sameApp {
            hotkeys.reset()
        }
        return (effectiveVE, session.activeApp)
    }

    private func makeSendOptionsInsideLock(activeApp: String) -> SendOptions {
        let delayMs = config.send_delay_ms > 0
            ? min(Int(config.send_delay_ms), 25)
            : (config.mechanism == .compatibleHook ? 1 : 0)
        let isBrowser = MacAppDetector.isBrowserOrSpotlight(activeApp)
        return SendOptions(
            useClipboard: config.use_clipboard_send || config.mechanism == .compatibleHook,
            pasteShiftInsert: config.paste_via_shift_insert,
            standardMode: config.standard_key_send || MacAppDetector.isTerminalApp(activeApp),
            delayMs: delayMs,
            browserFix1: config.fix_browser_suggest_1 && isBrowser,
            browserFix2: config.fix_browser_suggest_2 && isBrowser
        )
    }

    private func clearSelectionPopupFlag() {
        lock.lock()
        isSelectionPopupOpen = false
        lock.unlock()
    }

    public func triggerConvertSelection(fromMenuBar: Bool = false) {
        lock.lock()
        pasteGeneration &+= 1
        pendingSavedPasteboard = nil
        rust.resetAll()
        bufferActive = false
        lock.unlock()

        selectionQueue.async { [weak self] in
            guard let self else { return }
            if fromMenuBar {
                usleep(140_000)
            }
            let prep = self.runOnMainSync { () -> (pid_t, CGPoint, [[(String, Data)]], Int) in
                let pid = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0
                let anchor = MacAppDetector.anchorPointNearCaretOrCursor()
                let pb = NSPasteboard.general
                let saved = PasteboardSnapshot.capture(pb)
                let countBefore = pb.changeCount
                return (pid, anchor, saved, countBefore)
            }

            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_ANSI_C), flags: .maskCommand, delayMs: 0)

            var copied: String?
            var countAfterCopy = prep.3
            for _ in 0..<18 {
                usleep(10_000)
                let poll = self.runOnMainSync { () -> (Int, String?) in
                    let pb = NSPasteboard.general
                    return (pb.changeCount, pb.string(forType: .string))
                }
                if poll.0 != prep.3 {
                    countAfterCopy = poll.0
                    copied = poll.1
                    break
                }
            }

            guard countAfterCopy != prep.3, let text = copied, !text.isEmpty else {
                self.runOnMainSync {
                    let pb = NSPasteboard.general
                    if pb.changeCount != prep.3 {
                        PasteboardSnapshot.restore(prep.2, to: pb)
                    }
                }
                return
            }

            self.lock.lock()
            self.isSelectionPopupOpen = true
            self.lock.unlock()

            let targetPid = prep.0
            let anchor = prep.1
            let saved = prep.2

            Task { @MainActor [weak self] in
                guard let self else { return }
                SelectionActionPanel.shared.show(
                    previewText: text,
                    anchor: anchor,
                    onSelect: { [weak self] item in
                        guard let self else { return }
                        self.clearSelectionPopupFlag()
                        let transformed = RustEngine.convertWithEncoding(
                            text,
                            from: .auto,
                            to: .unicode,
                            caseTransform: item.caseTransform
                        )
                        let pb = NSPasteboard.general
                        pb.clearContents()
                        pb.setString(transformed, forType: .string)
                        let expectedPasteCount = pb.changeCount
                        if targetPid > 0,
                           NSWorkspace.shared.frontmostApplication?.processIdentifier != targetPid,
                           let app = NSRunningApplication(processIdentifier: targetPid) {
                            app.activate()
                        }
                        self.selectionQueue.async {
                            usleep(35_000)
                            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_ANSI_V), flags: .maskCommand, delayMs: 0)
                            usleep(145_000)
                            Task { @MainActor in
                                let pbNow = NSPasteboard.general
                                if pbNow.changeCount == expectedPasteCount {
                                    PasteboardSnapshot.restore(saved, to: pbNow)
                                }
                            }
                        }
                    },
                    onCancel: { [weak self] in
                        guard let self else { return }
                        self.clearSelectionPopupFlag()
                        let pb = NSPasteboard.general
                        if pb.changeCount == countAfterCopy {
                            PasteboardSnapshot.restore(saved, to: pb)
                        }
                    }
                )
            }
        }
    }

    fileprivate func handleEvent(
        proxy: CGEventTapProxy,
        type: CGEventType,
        event: CGEvent
    ) -> Unmanaged<CGEvent>? {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            lock.lock()
            rust.resetAll()
            rust.setTemporaryEnglish(false)
            hotkeys.reset()
            bufferActive = false
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
            rust.setTemporaryEnglish(false)
            bufferActive = false
            hotkeys.reset()
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        if type == .flagsChanged {
            return handleFlagsChanged(event)
        }
        if type == .keyDown {
            return handleKeyDown(event, proxy: proxy)
        }
        return Unmanaged.passUnretained(event)
    }

    private func handleFlagsChanged(_ event: CGEvent) -> Unmanaged<CGEvent>? {
        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let flags = event.flags.rawValue
        lock.lock()
        let sig = hotkeys.handleFlagsChanged(
            preset: config.switch_preset,
            flags: flags,
            quickCapslockSwitch: config.quickCapslockSwitch,
            keyCode: keyCode
        )
        if let holdState = hotkeys.updateHoldEnglish(holdKey: config.holdEnglishKey, flags: flags, keyCode: keyCode) {
            rust.setTemporaryEnglish(holdState)
        }
        let uiSync = sig == .toggleVE ? toggleVEInsideLock() : nil
        lock.unlock()
        if let uiSync {
            dispatchSyncUI(ve: uiSync.0, app: uiSync.1, beep: uiSync.2, userInitiated: true)
        }
        return Unmanaged.passUnretained(event)
    }

    private func handleKeyDown(_ event: CGEvent, proxy: CGEventTapProxy) -> Unmanaged<CGEvent>? {
        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let vk = MacKeycode.toPortableVK(keyCode)
        let flags = event.flags.rawValue

        lock.lock()
        let popupOpen = isSelectionPopupOpen
        lock.unlock()
        if popupOpen {
            let modMask = CGEventFlags.maskCommand.rawValue | CGEventFlags.maskControl.rawValue | CGEventFlags.maskAlternate.rawValue
            if Int(keyCode) == kVK_Escape {
                clearSelectionPopupFlag()
                Task { @MainActor in SelectionActionPanel.shared.cancel() }
                return nil
            }
            if (flags & modMask) == 0, let choice = MacKeycode.selectionDigit(for: keyCode) {
                clearSelectionPopupFlag()
                Task { @MainActor in SelectionActionPanel.shared.triggerShortcutKey(choice) }
                return nil
            }
            clearSelectionPopupFlag()
            Task { @MainActor in SelectionActionPanel.shared.cancel() }
        }

        let targetPID = pid_t(truncatingIfNeeded: event.getIntegerValueField(.eventTargetUnixProcessID))
        let frontPID = NSWorkspace.shared.frontmostApplication?.processIdentifier
        let secureInput = IsSecureEventInputEnabled()

        lock.lock()
        if let holdState = hotkeys.updateHoldEnglish(holdKey: config.holdEnglishKey, flags: flags, keyCode: nil) {
            rust.setTemporaryEnglish(holdState)
        }
        var pendingAppSync: (Bool, String)?
        let focus = typingFocus
        if let frontPID, focus?.pid != frontPID, requestedFocusPID != frontPID {
            requestedFocusPID = frontPID
            onFocusRefresh?()
        }
        if let focus {
            pendingAppSync = applyWindowCheckInsideLock(appName: focus.appName, windowTitle: focus.windowTitle)
        }
        let sig = hotkeys.handleKeyDown(
            preset: config.switch_preset,
            customSwitch: config.custom_switch_hotkey,
            convertHotkey: config.clipboard_convert_hotkey,
            vk: vk,
            flags: flags,
            undoHotkey: config.undoLastWordHotkey,
            convertSelectionHotkey: config.convertSelectionHotkey
        )
        if sig == .toggleVE {
            let uiSync = toggleVEInsideLock()
            lock.unlock()
            dispatchSyncUI(ve: uiSync.0, app: uiSync.1, beep: uiSync.2, userInitiated: true)
            return nil
        }
        if sig == .convertClipboard {
            rust.resetAll()
            bufferActive = false
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            DispatchQueue.main.async { self.onConvertClipboard?() }
            return nil
        }
        let skipFocus = focus?.allowsTyping(targetPID: targetPID, frontPID: frontPID, secureInput: secureInput) != true
        if skipFocus {
            rust.resetAll()
            bufferActive = false
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            return Unmanaged.passUnretained(event)
        }
        if sig == .convertSelection {
            rust.resetAll()
            bufferActive = false
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            triggerConvertSelection(fromMenuBar: false)
            return nil
        }
        if sig == .undoLastWord {
            let undoAct = rust.toggleLastCommittedWord()
            if undoAct.replace {
                bufferActive = false
                let undoOpts = makeSendOptionsInsideLock(activeApp: session.activeApp)
                lock.unlock()
                if let s = pendingAppSync {
                    DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
                }
                return applyAction(undoAct, original: event, triggerKeyCode: keyCode, opts: undoOpts, cursorLeft: 0, proxy: proxy)
            }
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            return Unmanaged.passUnretained(event)
        }

        let activeApp = session.activeApp
        let modMask = CGEventFlags.maskCommand.rawValue | CGEventFlags.maskControl.rawValue | CGEventFlags.maskAlternate.rawValue
        let hasAccelMod = (flags & modMask) != 0
        let inSandbox = sandboxFocused && focus?.pid == ProcessInfo.processInfo.processIdentifier
        if inSandbox || hasAccelMod || session.activeBehavior == .blacklist {
            if hasAccelMod {
                rust.resetWord()
                bufferActive = false
                pasteGeneration &+= 1
                pendingSavedPasteboard = nil
            }
            lock.unlock()
            if let s = pendingAppSync {
                DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
            }
            return Unmanaged.passUnretained(event)
        }

        let bypassLayout = config.disable_on_non_us_layout && !usesUSKeyboardLayout
        let opts = makeSendOptionsInsideLock(activeApp: activeApp)
        lock.unlock()

        if let s = pendingAppSync {
            DispatchQueue.main.async { self.onSyncUI?(s.0, s.1, false) }
        }
        if bypassLayout {
            return Unmanaged.passUnretained(event)
        }
        return processKeyAction(event: event, keyCode: keyCode, opts: opts, proxy: proxy)
    }

    private func processKeyAction(event: CGEvent, keyCode: UInt16, opts: SendOptions, proxy: CGEventTapProxy) -> Unmanaged<CGEvent>? {
        lock.lock()
        var cursorLeft = 0
        let act: KeyActionResult
        if Int(keyCode) == kVK_Escape {
            act = rust.processEscape()
            bufferActive = false
        } else if Int(keyCode) == kVK_Delete {
            let tryReedit = config.reeditSurroundingWord && effectiveVE && !bufferActive
            if tryReedit {
                lock.unlock()
                let surrounding = MacAppDetector.surroundingWordBeforeCaret()
                lock.lock()
                if let word = surrounding, word.count > 1 {
                    let remaining = String(word.dropLast())
                    rust.hydrateWord(remaining)
                    bufferActive = true
                    lock.unlock()
                    return Unmanaged.passUnretained(event)
                }
            }
            act = rust.processBackspace()
            if act.replace {
                cursorLeft = rust.takePendingCursorLeft()
            }
        } else if MacKeycode.isNavigationKey(keyCode) {
            rust.resetWord()
            bufferActive = false
            lock.unlock()
            return Unmanaged.passUnretained(event)
        } else if let scalar = Self.extractScalar(from: event, keyCode: keyCode) {
            let needsClipboard = config.macro_enabled
                && config.macros.contains(where: { $0.enabled && $0.expansion.contains("{clipboard}") })
                && rust.isActiveDelimiter(scalar: scalar)
            if needsClipboard {
                lock.unlock()
                let clip = runOnMainSync { NSPasteboard.general.string(forType: .string) ?? "" }
                lock.lock()
                rust.setClipboardText(clip)
            }
            let isDelim = rust.isActiveDelimiter(scalar: scalar) || scalar == 10 || scalar == 9
            act = rust.processKey(scalar: scalar)
            bufferActive = !isDelim
            if act.replace {
                cursorLeft = rust.takePendingCursorLeft()
            }
        } else {
            rust.resetWord()
            bufferActive = false
            lock.unlock()
            return Unmanaged.passUnretained(event)
        }
        lock.unlock()
        return applyAction(act, original: event, triggerKeyCode: keyCode, opts: opts, cursorLeft: cursorLeft, proxy: proxy)
    }

    private func toggleVEInsideLock() -> (Bool, String, Bool) {
        let next = !effectiveVE
        effectiveVE = next
        config.vietnamese_enabled = next
        session.setUserVE(next)
        rust.setEnabled(next)
        rust.resetAll()
        rust.setTemporaryEnglish(false)
        bufferActive = false
        return (next, session.activeApp, config.beep_on_switch)
    }

    private func dispatchSyncUI(ve: Bool, app: String, beep: Bool, userInitiated: Bool) {
        DispatchQueue.main.async {
            if beep { NSSound.beep() }
            self.onSyncUI?(ve, app, userInitiated)
        }
    }

    static func extractScalar(from event: CGEvent, keyCode: UInt16) -> UInt32? {
        if Int(keyCode) == kVK_Return || Int(keyCode) == kVK_ANSI_KeypadEnter { return 10 }
        if Int(keyCode) == kVK_Tab { return 9 }
        var length = 0
        var chars = [UniChar](repeating: 0, count: 4)
        event.keyboardGetUnicodeString(maxStringLength: 4, actualStringLength: &length, unicodeString: &chars)
        guard (1...2).contains(length) else { return nil }
        let scalars = String(decoding: chars.prefix(length), as: UTF16.self).unicodeScalars
        return scalars.count == 1 ? scalars.first?.value : nil
    }

    private func applyAction(
        _ act: KeyActionResult,
        original: CGEvent,
        triggerKeyCode: UInt16,
        opts: SendOptions,
        cursorLeft: Int,
        proxy: CGEventTapProxy
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
                CGEventEmitter.postUnicodeText("\u{200B}", paceEachKey: true, delayMs: opts.delayMs) { $0.tapPostEvent(proxy) }
                CGEventEmitter.postBackspaces(1, delayMs: opts.delayMs, proxy: proxy)
            }
            if opts.browserFix2 {
                CGEventEmitter.postVirtualKey(CGKeyCode(kVK_RightArrow), flags: [], delayMs: opts.delayMs, proxy: proxy)
            }
            CGEventEmitter.postBackspaces(act.backspaces, delayMs: opts.delayMs, proxy: proxy)
        }
        if !body.isEmpty {
            if opts.useClipboard && body.count > 1 {
                emitViaPasteboard(body, pasteShiftInsert: opts.pasteShiftInsert, delayMs: opts.delayMs, proxy: proxy)
            } else {
                CGEventEmitter.postUnicodeText(body, paceEachKey: opts.standardMode, delayMs: opts.delayMs) { $0.tapPostEvent(proxy) }
            }
        }
        if let trailingVK {
            if opts.useClipboard && body.count > 1 { usleep(8000) }
            CGEventEmitter.postVirtualKey(trailingVK, flags: [], delayMs: opts.delayMs, proxy: proxy)
        } else if cursorLeft > 0 && opts.useClipboard && body.count > 1 {
            usleep(8000)
        }
        if cursorLeft > 0 {
            for _ in 0..<cursorLeft {
                CGEventEmitter.postVirtualKey(CGKeyCode(kVK_LeftArrow), flags: [], delayMs: opts.delayMs, proxy: proxy)
            }
        }
        return nil
    }

    private func emitViaPasteboard(_ text: String, pasteShiftInsert: Bool, delayMs: Int, proxy: CGEventTapProxy) {
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
            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_Help), flags: .maskShift, delayMs: 0, proxy: proxy)
        } else {
            CGEventEmitter.postVirtualKey(CGKeyCode(kVK_ANSI_V), flags: .maskCommand, delayMs: 0, proxy: proxy)
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
