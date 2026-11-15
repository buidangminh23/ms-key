import AppKit
import InstallerCore
import SwiftUI

@MainActor
final class InstallerModel: ObservableObject {
    @Published var destination: URL
    @Published var phase: InstallPhase = .preparing
    @Published var completedBytes: Int64 = 0
    @Published var totalBytes: Int64 = 0
    @Published var isInstalling = false
    @Published var isCancelling = false
    @Published var installedApp: URL?
    @Published var errorMessage: String?
    private var cancellation: InstallCancellation?
    private var operationID = UUID()
    let source: URL

    init(source: URL) {
        self.source = source
        destination = FileManager.default.isWritableFile(atPath: "/Applications")
            ? URL(fileURLWithPath: "/Applications", isDirectory: true)
            : FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications", isDirectory: true)
    }

    var version: String {
        Bundle(url: source)?.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    var status: String {
        if installedApp != nil { return "MS Key đã sẵn sàng" }
        if isCancelling { return "Đang hủy và dọn tệp tạm…" }
        guard isInstalling else { return errorMessage == nil ? "Sẵn sàng cài đặt" : "Chưa cài đặt được" }
        switch phase {
        case .preparing: return "Đang kiểm tra ứng dụng…"
        case .copying: return "Đang chép vào Applications…"
        case .verifying: return "Đang xác minh bản đã chép…"
        case .finishing: return "Đang hoàn tất cài đặt…"
        }
    }

    var byteDescription: String {
        let completed = ByteCountFormatter.string(fromByteCount: completedBytes, countStyle: .file)
        let total = ByteCountFormatter.string(fromByteCount: totalBytes, countStyle: .file)
        return "\(completed) / \(total)"
    }

    func chooseDestination() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.directoryURL = destination
        panel.prompt = "Chọn thư mục"
        if panel.runModal() == .OK, let url = panel.url { destination = url }
    }

    func install() {
        guard !isInstalling else { return }
        errorMessage = nil
        installedApp = nil
        isInstalling = true
        isCancelling = false
        completedBytes = 0
        totalBytes = 0
        phase = .preparing
        let cancellation = InstallCancellation()
        self.cancellation = cancellation
        let source = source
        let destination = destination
        let model = self
        let operationID = UUID()
        self.operationID = operationID
        Task {
            do {
                let installed = try await Task.detached(priority: .userInitiated) {
                    try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
                    return try AppInstaller().install(source: source, destinationDirectory: destination, cancellation: cancellation) { value in
                        Task { @MainActor in
                            guard model.operationID == operationID, model.isInstalling, !model.isCancelling else { return }
                            model.phase = value.phase
                            model.completedBytes = value.completedBytes
                            model.totalBytes = value.totalBytes
                        }
                    }
                }.value
                installedApp = installed
            } catch {
                if case InstallError.cancelled = error {
                    errorMessage = "Đã hủy cài đặt. Bản MS Key hiện có được giữ nguyên."
                } else {
                    errorMessage = error.localizedDescription
                }
            }
            isInstalling = false
            isCancelling = false
            self.cancellation = nil
        }
    }

    func cancel() {
        guard isInstalling, phase != .finishing else { return }
        isCancelling = true
        cancellation?.cancel()
    }

    func openInstalledApp() {
        guard let installedApp else { return }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.arguments = ["--show"]
        NSWorkspace.shared.openApplication(at: installedApp, configuration: configuration) { _, error in
            if let error {
                Task { @MainActor in self.errorMessage = error.localizedDescription }
            }
        }
    }

    func revealInstalledApp() {
        if let installedApp { NSWorkspace.shared.activateFileViewerSelecting([installedApp]) }
    }
}
