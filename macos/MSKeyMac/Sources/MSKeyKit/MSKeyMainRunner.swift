import AppKit
import Foundation
import SwiftUI

public enum ActiveSheetKind: String, Identifiable, Sendable {
    case macros
    case customKeymap
    case appRules
    case converter
    case extras
    case userDict
    case captureSwitchHotkey
    case captureConvertHotkey

    public var id: String { rawValue }
}

@MainActor
public final class MSKeyStore: ObservableObject {
    @Published public var config: MSKeyConfig
    @Published public var effectiveVE: Bool
    @Published public var lastActiveApp: String = ""
    @Published public var activeSheet: ActiveSheetKind?
    @Published public var accessibilityGranted: Bool

    public let rust: RustEngine
    public let tapManager: EventTapManager
    public var menuBar: MenuBarController?
    private var mainWindow: NSWindow?
    private var workspaceObserver: NSObjectProtocol?
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
                        MacOSDHud.shared.show(vietnamese: ve)
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
        let current = MacAppDetector.frontmostAppIdentifier()
        if !current.isEmpty {
            lastActiveApp = current
            tapManager.notifyAppActivated(current)
        }
        workspaceObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] note in
            let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            let name = app.map(MacAppDetector.canonicalName(for:)) ?? ""
            DispatchQueue.main.async {
                guard let self, !name.isEmpty else { return }
                self.tapManager.notifyAppActivated(name)
            }
        }
        if !tapManager.start() {
            startPermissionWatch()
        }
        if config.show_on_startup {
            showMainWindow()
        }
    }

    private func startPermissionWatch() {
        permTimer?.invalidate()
        permTimer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] _ in
            DispatchQueue.main.async {
                guard let self else { return }
                let ok = EventTapManager.hasAccessibilityPermission(prompt: false)
                self.accessibilityGranted = ok
                if ok {
                    _ = self.tapManager.start()
                    self.permTimer?.invalidate()
                    self.permTimer = nil
                }
            }
        }
    }

    public func requestAccessibility() {
        accessibilityGranted = EventTapManager.hasAccessibilityPermission(prompt: true)
        if accessibilityGranted {
            _ = tapManager.start()
        } else {
            startPermissionWatch()
        }
    }

    public func toggleVE() {
        let next = !effectiveVE
        effectiveVE = next
        updateConfig { $0.vietnamese_enabled = next }
        if config.beep_on_switch {
            NSSound.beep()
        }
        if config.show_switch_osd {
            MacOSDHud.shared.show(vietnamese: next)
        }
    }

    public func updateConfig(_ mutate: (inout MSKeyConfig) -> Void) {
        let prevAuto = config.auto_start
        mutate(&config)
        effectiveVE = config.vietnamese_enabled
        tapManager.updateConfig(config)
        MacConfigStore.save(config)
        menuBar?.updateStatusIcon(vietnamese: effectiveVE)
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
    }

    public func addLastAppForceEnglish() {
        let app = lastActiveApp.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !app.isEmpty else { return }
        updateConfig { cfg in
            if let idx = cfg.app_rules.firstIndex(where: { $0.process_name.caseInsensitiveCompare(app) == .orderedSame }) {
                cfg.app_rules[idx].behavior = .forceEnglish
            } else {
                cfg.app_rules.append(AppRuleItem(process_name: app, behavior: .forceEnglish))
            }
        }
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
        if let win = mainWindow {
            applyTheme(config.theme)
            win.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        let host = NSHostingController(rootView: SettingsView(store: self))
        let win = NSWindow(contentViewController: host)
        win.title = "MS Key 0.1"
        win.styleMask = [.titled, .closable, .miniaturizable]
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
            let report = "ffi_typed=\(typed)\npanel=560x568\n"
            try? report.write(toFile: outPath, atomically: true, encoding: .utf8)
            return
        }
        let app = NSApplication.shared
        let delegate = MSKeyAppDelegate(openConverterOnLaunch: args.contains("--convert"))
        app.delegate = delegate
        app.run()
    }
}
