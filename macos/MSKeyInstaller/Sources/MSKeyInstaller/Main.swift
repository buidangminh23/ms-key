import AppKit
import InstallerCore
import SwiftUI

@MainActor
final class InstallerDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let model: InstallerModel
    private var window: NSWindow?

    init(source: URL) { model = InstallerModel(source: source) }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let host = NSHostingController(rootView: InstallerView(model: model))
        let window = NSWindow(contentViewController: host)
        window.title = "Install MS Key"
        window.styleMask = [.titled, .closable, .miniaturizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        window.makeKeyAndOrderFront(nil)
        self.window = window
        NSApp.activate(ignoringOtherApps: true)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if model.isInstalling { model.cancel(); return false }
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if model.isInstalling { model.cancel(); return .terminateCancel }
        return .terminateNow
    }
}

@main
enum InstallerMain {
    @MainActor
    static func main() {
        let args = CommandLine.arguments
        if let index = args.firstIndex(of: "--verify-install"), args.count == index + 3 {
            do {
                let destination = try AppInstaller().install(
                    source: URL(fileURLWithPath: args[index + 1]),
                    destinationDirectory: URL(fileURLWithPath: args[index + 2]),
                    cancellation: InstallCancellation(),
                    onProgress: { value in print("\(value.phase): \(value.completedBytes)/\(value.totalBytes)") }
                )
                print("Installed: \(destination.path)")
            } catch {
                FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
                exit(1)
            }
            return
        }
        let source = Bundle.main.resourceURL!.appendingPathComponent("MSKey.app", isDirectory: true)
        let app = NSApplication.shared
        if let index = args.firstIndex(of: "--verify-ui"), args.count == index + 2 {
            do {
                try renderPreviews(source: source, directory: URL(fileURLWithPath: args[index + 1]))
            } catch {
                FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
                exit(1)
            }
            return
        }
        app.setActivationPolicy(.regular)
        let delegate = InstallerDelegate(source: source)
        app.delegate = delegate
        withExtendedLifetime(delegate) { app.run() }
    }

    @MainActor
    private static func renderPreviews(source: URL, directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for state in ["ready", "copying", "complete", "error"] {
            let model = InstallerModel(source: source)
            if state == "copying" {
                model.isInstalling = true
                model.phase = .copying
                model.totalBytes = 10_000_000
                model.completedBytes = 6_500_000
            } else if state == "complete" {
                model.installedApp = model.destination.appendingPathComponent("MSKey.app")
            } else if state == "error" {
                model.errorMessage = "Hãy thoát MS Key đang chạy trước khi cập nhật."
            }
            let host = NSHostingView(rootView: InstallerView(model: model).environment(\.colorScheme, .light))
            let frame = NSRect(origin: .zero, size: host.fittingSize)
            let window = NSWindow(contentRect: frame, styleMask: .borderless, backing: .buffered, defer: false)
            window.contentView = host
            host.frame = frame
            host.layoutSubtreeIfNeeded()
            host.displayIfNeeded()
            guard let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) else {
                throw CocoaError(.fileWriteUnknown)
            }
            host.cacheDisplay(in: host.bounds, to: bitmap)
            guard let png = bitmap.representation(using: .png, properties: [:]) else {
                throw CocoaError(.fileWriteUnknown)
            }
            try png.write(to: directory.appendingPathComponent("\(state).png"))
        }
        print("Rendered installer states: ready, copying, complete, error")
    }
}
