import AppKit
import Carbon
import Foundation
import SwiftUI

public enum ActiveSheetKind: String, Identifiable, Sendable {
    case macros
    case customKeymap
    case appRules
    case converter
    case sandbox
    case extras
    case userDict
    case captureSwitchHotkey
    case captureConvertHotkey
    case captureUndoHotkey
    case captureConvertSelectionHotkey

    public var id: String { rawValue }
}

@MainActor
public final class MSKeyStore: ObservableObject {
    @Published public var config: MSKeyConfig
    @Published public var effectiveVE: Bool
    @Published public var lastActiveApp: String = ""
    @Published public var activeSheet: ActiveSheetKind?
    @Published public var accessibilityGranted: Bool
    @Published public var presentationID = 0

    public let rust: RustEngine
    public let tapManager: EventTapManager
    public var menuBar: MenuBarController?
    private var mainWindow: NSWindow?
    private var workspaceObserver: NSObjectProtocol?
    private var inputSourceObserver: NSObjectProtocol?
    private var permTimer: Timer?

    public init() {
        let loaded = MacConfigStore.load()
        self.config = loaded
        self.effectiveVE = loaded.vietnamese_enabled
        self.accessibilityGranted = EventTapManager.hasAccessibilityPermission(prompt: false)
        let enc = JSONEncoder()
        let initialJSON = (try? enc.encode(loaded)).flatMap { String(data: $0, encoding: .utf8) }
        self.rust = RustEngine(configJSON: initialJSON)
        self.tapManager = EventTapManager(rust: self.rust, config: loaded)
        self.tapManager.onSyncUI = { [weak self] ve, app, userInitiated in
            DispatchQueue.main.async {
                guard let self else { return }
                self.effectiveVE = ve
                if !app.isEmpty { self.lastActiveApp = app }
                if userInitiated {
                    self.config.vietnamese_enabled = ve
                    MacConfigStore.save(self.config)
                    if self.config.show_switch_osd {
                        MacOSDHud.shared.show(vietnamese: ve, nearCaret: self.config.showCaretOsd)
                    }
                }
                self.menuBar?.updateStatusIcon(vietnamese: ve)
            }
        }
        self.tapManager.onConvertClipboard = { [weak self] in
            DispatchQueue.main.async {
                self?.quickConvertPasteboard()
            }
        }
    }

    public func startRuntime() {
        menuBar = MenuBarController(store: self)
        let current = MacAppDetector.frontmostAppAndWindowTitle()
        if !current.appName.isEmpty {
            lastActiveApp = current.appName
            tapManager.notifyAppActivated(current.appName, windowTitle: current.windowTitle)
        }
        workspaceObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] note in
            let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            let name = app.map(MacAppDetector.canonicalName(for:)) ?? ""
            let pid = app?.processIdentifier ?? 0
            DispatchQueue.main.async {
                guard let self, !name.isEmpty else { return }
                let title = AXIsProcessTrusted() && pid > 0
                    ? MacAppDetector.focusedWindowTitle(of: AXUIElementCreateApplication(pid))
                    : ""
                self.tapManager.notifyAppActivated(name, windowTitle: title)
            }
        }
        inputSourceObserver = DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name(kTISNotifySelectedKeyboardInputSourceChanged as String),
            object: nil,
            queue: .main
        ) { [weak self] _ in
            DispatchQueue.main.async {
                self?.refreshKeyboardLayout()
            }
        }
        refreshTapHealth()
        if !accessibilityGranted {
            promptAccessibilityOncePerVersion()
        }
        startTapHealthWatch()
        if config.show_on_startup {
            showMainWindow()
        }
        if config.checkUpdatesOnStart {
            DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(1)) {
                UpdateChecker.shared.checkForUpdates(manual: false)
            }
        }
    }

    private func startTapHealthWatch() {
        permTimer?.invalidate()
        let timer = Timer(timeInterval: 2.0, repeats: true) { [weak self] _ in
            DispatchQueue.main.async {
                self?.refreshTapHealth()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        permTimer = timer
    }

    private func refreshKeyboardLayout() {
        tapManager.updateKeyboardLayout(isUS: MacAppDetector.isUSKeyboardLayout())
    }

    public func refreshTapHealth() {
        refreshKeyboardLayout()
        let trusted = EventTapManager.hasAccessibilityPermission(prompt: false)
        switch EventTapManager.healthAction(trusted: trusted, state: tapManager.tapState) {
        case .start:
            tapManager.start()
        case .recover:
            tapManager.recover()
        case .stop:
            tapManager.stop()
        case .none:
            break
        }
        if accessibilityGranted != trusted {
            accessibilityGranted = trusted
        }
        menuBar?.setTypingReady(trusted && tapManager.tapState == .enabled)
    }

    private func promptAccessibilityOncePerVersion() {
        let key = "mskey.accessibilityPromptVersion"
        let version = MSKeyBrand.version
        guard UserDefaults.standard.string(forKey: key) != version else { return }
        UserDefaults.standard.set(version, forKey: key)
        _ = EventTapManager.hasAccessibilityPermission(prompt: true)
    }

    public func requestAccessibility() {
        _ = EventTapManager.hasAccessibilityPermission(prompt: true)
        refreshTapHealth()
    }

    public static func openAccessibilitySettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") else {
            return
        }
        NSWorkspace.shared.open(url)
    }

    public func toggleVE() {
        let next = !effectiveVE
        effectiveVE = next
        updateConfig { $0.vietnamese_enabled = next }
        if config.beep_on_switch {
            NSSound.beep()
        }
        if config.show_switch_osd {
            MacOSDHud.shared.show(vietnamese: next, nearCaret: config.showCaretOsd)
        }
    }

    public func updateConfig(_ mutate: (inout MSKeyConfig) -> Void) {
        let prevAuto = config.auto_start
        mutate(&config)
        effectiveVE = config.vietnamese_enabled
        tapManager.updateConfig(config)
        MacConfigStore.save(config)
        menuBar?.updateStatusIcon(vietnamese: effectiveVE)
        menuBar?.refreshProfilesMenu()
        applyTheme(config.theme)
        if config.auto_start != prevAuto {
            MacConfigStore.syncAutoStart(enable: config.auto_start)
        }
    }

    public func resetDefaults() {
        updateConfig { $0 = MSKeyConfig() }
    }

    public func saveNow() {
        rust.syncClockOffset()
        effectiveVE = config.vietnamese_enabled
        tapManager.updateConfig(config)
        MacConfigStore.save(config)
        MacConfigStore.syncAutoStart(enable: config.auto_start)
        menuBar?.updateStatusIcon(vietnamese: effectiveVE)
        menuBar?.refreshProfilesMenu()
    }

    @discardableResult
    public func applyProfile(_ name: String) -> Bool {
        rust.setConfig(config)
        guard rust.applyProfile(name) else { return false }
        let json = rust.getConfigJSON()
        guard let data = json.data(using: .utf8),
              let decoded = try? JSONDecoder().decode(MSKeyConfig.self, from: data) else {
            return false
        }
        updateConfig { $0 = decoded }
        return true
    }

    public func saveCurrentAsProfile(named rawName: String) {
        let trimmed = rawName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        updateConfig { $0.saveCurrentAsProfile(trimmed) }
    }

    public func deleteCustomProfile(named name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        updateConfig { cfg in
            cfg.profiles.removeAll { $0.name.caseInsensitiveCompare(trimmed) == .orderedSame }
            if cfg.activeProfile.caseInsensitiveCompare(trimmed) == .orderedSame {
                cfg.activeProfile = "Mặc định"
            }
        }
    }

    public func addLastAppForceEnglish() {
        let app = lastActiveApp.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !app.isEmpty else { return }
        updateConfig { cfg in
            if let idx = cfg.app_rules.firstIndex(where: {
                $0.process_name.caseInsensitiveCompare(app) == .orderedSame && $0.titlePattern.isEmpty
            }) {
                cfg.app_rules[idx].behavior = .forceEnglish
            } else {
                cfg.app_rules.append(AppRuleItem(process_name: app, titlePattern: "", behavior: .forceEnglish))
            }
        }
    }

    @discardableResult
    public func addLastEscWordToUserDict() -> String? {
        guard let raw = rust.lastWordForDict()?.trimmingCharacters(in: .whitespacesAndNewlines),
              !raw.isEmpty else {
            return nil
        }
        updateConfig { cfg in
            if !cfg.user_dict_words.contains(where: { $0.caseInsensitiveCompare(raw) == .orderedSame }) {
                cfg.user_dict_words.append(raw)
            }
        }
        return raw
    }

    public func chooseSyncConfigPath() {
        let panel = NSOpenPanel()
        panel.title = "Chọn file JSON hoặc thư mục đồng bộ cấu hình"
        panel.canChooseFiles = true
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        var isDir: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir)
        let chosenURL = (exists && isDir.boolValue)
            ? url.appendingPathComponent("mskey-config.json")
            : url
        updateConfig { $0.syncConfigPath = chosenURL.path }
        let synced = MacConfigStore.load()
        if synced != config {
            updateConfig { $0 = synced }
        }
    }

    public func clearSyncConfigPath() {
        updateConfig { $0.syncConfigPath = "" }
    }

    public func triggerConvertSelection() {
        tapManager.triggerConvertSelection(fromMenuBar: true)
    }

    public func checkForUpdates(manual: Bool = true) {
        UpdateChecker.shared.checkForUpdates(manual: manual)
    }

    public func quickConvertPasteboard() {
        let pb = NSPasteboard.general
        guard let raw = pb.string(forType: .string), !raw.isEmpty else { return }
        let out = RustEngine.convertWithEncoding(raw, from: .auto, to: .unicode, caseMode: 0)
        pb.clearContents()
        pb.setString(out, forType: .string)
    }

    public func importEVKeyTreePanel() {
        let panel = NSOpenPanel()
        panel.title = "Chọn file EVKey setting.tree"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url,
              let text = try? String(contentsOf: url, encoding: .utf8),
              let imported = RustEngine.importEVKeyTree(text) else {
            return
        }
        updateConfig { $0 = imported }
    }

    public func openSheet(_ kind: ActiveSheetKind) {
        showMainWindow()
        activeSheet = kind
    }

    public func showMainWindow() {
        if mainWindow?.isVisible != true { presentationID += 1 }
        if let win = mainWindow {
            applyTheme(config.theme)
            win.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        let host = NSHostingController(rootView: SettingsView(store: self))
        let win = NSWindow(contentViewController: host)
        win.title = "MS Key \(MSKeyBrand.version)"
        win.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        win.contentMinSize = SettingsView.minimumSize
        win.setContentSize(SettingsView.defaultSize)
        win.isReleasedWhenClosed = false
        win.center()
        mainWindow = win
        applyTheme(config.theme)
        win.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    public func closeMainWindow() {
        mainWindow?.orderOut(nil)
    }

    public func applyTheme(_ mode: ThemeModeKind) {
        let app: NSAppearance?
        switch mode {
        case .system: app = nil
        case .light: app = NSAppearance(named: .aqua)
        case .dark: app = NSAppearance(named: .darkAqua)
        }
        mainWindow?.appearance = app
    }
}

@MainActor
public final class MSKeyAppDelegate: NSObject, NSApplicationDelegate {
    private let store = MSKeyStore()
    private let openConverterOnLaunch: Bool

    public init(openConverterOnLaunch: Bool) {
        self.openConverterOnLaunch = openConverterOnLaunch
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        store.startRuntime()
        if CommandLine.arguments.contains("--show") { store.showMainWindow() }
        if openConverterOnLaunch {
            store.openSheet(.converter)
        }
    }

    public func applicationWillTerminate(_ notification: Notification) {
        store.tapManager.stop()
    }
}

@MainActor
public enum MSKeyMainRunner {
    public static func run() {
        let args = CommandLine.arguments
        if args.contains("--print-default-config") {
            print(RustEngine.defaultConfigJSON())
            return
        }
        if let idx = args.firstIndex(of: "--verify-smoke") {
            let outPath = idx + 1 < args.count ? args[idx + 1] : "/tmp/mskey-mac-smoke.txt"
            let engine = RustEngine()
            var typed = ""
            for scalar in "tieengs Vieejt ".unicodeScalars {
                let res = engine.processKey(scalar: scalar.value)
                if res.replace {
                    for _ in 0..<res.backspaces {
                        if !typed.isEmpty { typed.removeLast() }
                    }
                    typed.append(res.text)
                } else {
                    typed.append(Character(scalar))
                }
            }
            let app = NSApplication.shared
            app.setActivationPolicy(.prohibited)
            guard let markURL = Bundle.main.url(forResource: "mskey-mark", withExtension: "png"),
                  NSImage(contentsOf: markURL) != nil,
                  Bundle.main.url(forResource: "mskey", withExtension: "icns") != nil else {
                fputs("MS Key branding resources are missing\n", stderr)
                exit(1)
            }
            let host = NSHostingView(rootView: SettingsView(store: MSKeyStore()))
            host.frame = NSRect(origin: .zero, size: SettingsView.defaultSize)
            host.layoutSubtreeIfNeeded()
            let report = "ffi_typed=\(typed)\npanel=\(Int(host.bounds.width))x\(Int(host.bounds.height))\nbranding=loaded\n"
            try? report.write(toFile: outPath, atomically: true, encoding: .utf8)
            return
        }
        let app = NSApplication.shared
        let delegate = MSKeyAppDelegate(openConverterOnLaunch: args.contains("--convert"))
        app.delegate = delegate
        app.run()
    }
}
