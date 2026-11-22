import AppKit
import ApplicationServices
import Foundation

struct MacTypingFocus: Equatable {
    let pid: pid_t
    let appName: String
    let windowTitle: String
    let secureField: Bool

    func allowsTyping(targetPID: pid_t, frontPID: pid_t?, secureInput: Bool) -> Bool {
        !secureInput && !secureField && frontPID == pid && (targetPID <= 0 || targetPID == pid)
    }
}

enum MacFocusSubscription: Equatable {
    case pending
    case active
    case unsupported

    init(_ status: AXError) {
        switch status {
        case .success, .notificationAlreadyRegistered: self = .active
        case .notificationUnsupported, .notImplemented: self = .unsupported
        default: self = .pending
        }
    }
}

@MainActor
final class MacTypingFocusMonitor {
    private let tap: EventTapManager
    private var pid: pid_t?
    private var name = ""
    private var application: AXUIElement?
    private var observer: AXObserver?
    private var window: AXUIElement?
    private var field: AXUIElement?
    private var ready = false
    private var fieldSubscription = MacFocusSubscription.pending
    private var windowSubscription = MacFocusSubscription.pending
    private var warmup: Task<Void, Never>?
    private var titleSubscription = MacFocusSubscription.pending
    private var checkedTitleRules = false
    private let timeout: Float = 0.1

    init(tap: EventTapManager) {
        self.tap = tap
    }

    func synchronize(trusted: Bool, activatedApp: NSRunningApplication? = nil) {
        guard trusted else {
            if pid != nil { detach() }
            return
        }
        guard let app = activatedApp ?? NSWorkspace.shared.frontmostApplication else { return }
        if pid != app.processIdentifier {
            detach()
            attach(app)
            warmup = Task { [weak self] in
                for delay in [30, 100, 250, 500] {
                    do { try await Task.sleep(for: .milliseconds(delay)) } catch { return }
                    guard let self, self.pid == app.processIdentifier, !self.ready else { return }
                    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == app.processIdentifier else { continue }
                    if self.observer == nil { self.attach(app) } else { self.refresh() }
                }
            }
        } else if observer == nil {
            attach(app)
        } else if !ready || fieldSubscription != .active || windowSubscription == .pending ||
                    (tap.hasWindowTitleRules && (!checkedTitleRules || windowSubscription != .active || titleSubscription != .active)) {
            refresh()
        }
    }

    private func detach() {
        warmup?.cancel()
        warmup = nil
        if let observer {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .commonModes)
        }
        observer = nil
        application = nil
        window = nil
        field = nil
        fieldSubscription = .pending
        windowSubscription = .pending
        titleSubscription = .pending
        pid = nil
        ready = false
        tap.updateTypingFocus(nil, targetChanged: true)
    }

    private func subscribe(_ element: AXUIElement, _ notice: String, observer: AXObserver) -> MacFocusSubscription {
        MacFocusSubscription(AXObserverAddNotification(observer, element, notice as CFString, Unmanaged.passUnretained(self).toOpaque()))
    }

    private func attach(_ app: NSRunningApplication) {
        let processID = app.processIdentifier
        pid = processID
        name = MacAppDetector.canonicalName(for: app)
        let target = AXUIElementCreateApplication(processID)
        AXUIElementSetMessagingTimeout(target, timeout)
        var newObserver: AXObserver?
        let callback: AXObserverCallback = { sender, element, notice, pointer in
            guard let pointer else { return }
            MainActor.assumeIsolated {
                let monitor = Unmanaged<MacTypingFocusMonitor>.fromOpaque(pointer).takeUnretainedValue()
                monitor.notified(sender, element: element, notice as String)
            }
        }
        guard AXObserverCreate(processID, callback, &newObserver) == .success, let newObserver else { return }
        observer = newObserver
        application = target
        CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(newObserver), .commonModes)
        refresh()
    }

    private func notified(_ sender: AXObserver, element: AXUIElement, _ notice: String) {
        guard let observer, CFEqual(sender, observer) else { return }
        if notice == kAXTitleChangedNotification && !tap.hasWindowTitleRules {
            checkedTitleRules = false
            return
        }
        if ready && ((notice == kAXFocusedUIElementChangedNotification && element == field) ||
                     (notice == kAXFocusedWindowChangedNotification && element == window)) { return }
        ready = false
        tap.updateTypingFocus(nil, targetChanged: false)
        refresh()
    }

    private func attribute(_ key: String, of element: AXUIElement) -> (AXError, CFTypeRef?) {
        var result: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(element, key as CFString, &result)
        return (error, result)
    }

    private func child(_ key: String, of element: AXUIElement) -> (AXError, AXUIElement?) {
        let (error, value) = attribute(key, of: element)
        guard error == .success, let value, CFGetTypeID(value) == AXUIElementGetTypeID() else { return (error, nil) }
        let child = value as! AXUIElement
        AXUIElementSetMessagingTimeout(child, timeout)
        return (.success, child)
    }

    private func windowTitle(_ window: AXUIElement?, status: AXError) -> String? {
        guard let window else { return status == .noValue || status == .attributeUnsupported ? "" : nil }
        let (error, title) = attribute(kAXTitleAttribute, of: window)
        if error == .noValue || error == .attributeUnsupported { return "" }
        return error == .success ? title as? String : nil
    }

    nonisolated static func securityWithoutField(_ status: AXError, hasWindow: Bool) -> Bool? {
        guard hasWindow else { return nil }
        switch status {
        case .noValue, .attributeUnsupported, .notImplemented: return false
        default: return nil
        }
    }

    private func refresh() {
        guard let application, let pid, let observer else { return }
        let (windowStatus, nextWindow) = child(kAXFocusedWindowAttribute, of: application)
        let (fieldStatus, nextField) = child(kAXFocusedUIElementAttribute, of: application)
        if fieldSubscription == .pending && fieldStatus != .cannotComplete {
            fieldSubscription = subscribe(application, kAXFocusedUIElementChangedNotification, observer: observer)
        }
        if windowSubscription == .pending && windowStatus != .cannotComplete {
            windowSubscription = subscribe(application, kAXFocusedWindowChangedNotification, observer: observer)
        }
        let fieldChanged = fieldStatus != .cannotComplete && nextField != field
        let windowChanged = windowStatus != .cannotComplete && nextWindow != window
        let targetChanged = fieldChanged || windowChanged
        if ready && !targetChanged && fieldSubscription == .active &&
            (fieldStatus == .cannotComplete || windowStatus == .cannotComplete) { return }
        if windowChanged {
            if let window { AXObserverRemoveNotification(observer, window, kAXTitleChangedNotification as CFString) }
            window = nextWindow
            titleSubscription = .pending
        }
        if titleSubscription == .pending, let nextWindow {
            titleSubscription = subscribe(nextWindow, kAXTitleChangedNotification, observer: observer)
        }
        if fieldStatus != .cannotComplete { field = nextField }
        let nextFocus: MacTypingFocus?
        checkedTitleRules = tap.hasWindowTitleRules
        let title = windowTitle(nextWindow, status: windowStatus)
        let security = nextField.map(MacAppDetector.secureFieldStatus) ?? Self.securityWithoutField(fieldStatus, hasWindow: nextWindow != nil)
        if let secure = security,
           let title = title ?? (checkedTitleRules ? nil : ""),
           NSWorkspace.shared.frontmostApplication?.processIdentifier == pid {
            nextFocus = MacTypingFocus(pid: pid, appName: name, windowTitle: title, secureField: secure)
        } else {
            nextFocus = nil
        }
        ready = nextFocus != nil
        tap.updateTypingFocus(nextFocus, targetChanged: targetChanged)
    }
}
