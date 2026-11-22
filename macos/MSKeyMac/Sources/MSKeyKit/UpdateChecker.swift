import AppKit
import Combine
import CryptoKit
import Darwin
import Foundation

public struct GitHubReleaseAsset: Equatable, Sendable {
    public let name: String
    public let downloadURL: String
    public let size: UInt64

    public init(name: String, downloadURL: String, size: UInt64 = 0) {
        self.name = name
        self.downloadURL = downloadURL
        self.size = size
    }
}

public struct GitHubReleaseInfo: Equatable, Sendable {
    public let tagName: String
    public let version: String
    public let htmlURL: String
    public let body: String
    public let assets: [GitHubReleaseAsset]

    public init(
        tagName: String,
        version: String,
        htmlURL: String,
        body: String,
        assets: [GitHubReleaseAsset]
    ) {
        self.tagName = tagName
        self.version = version
        self.htmlURL = htmlURL
        self.body = body
        self.assets = assets
    }
}

public enum UpdatePhase: Equatable, Sendable {
    case idle, checking, upToDate, available, downloading, verifying, installing, restarting, downloaded, failed
}

@MainActor
public final class UpdateChecker: ObservableObject {
    public static let shared = UpdateChecker()
    nonisolated public static let latestReleaseAPI = "https://api.github.com/repos/buidangminh23/ms-key/releases/latest"
    nonisolated public static let releasesPageURL = "https://github.com/buidangminh23/ms-key/releases"

    @Published public private(set) var phase: UpdatePhase = .idle
    @Published public private(set) var statusText = ""
    @Published public private(set) var release: GitHubReleaseInfo?
    @Published public private(set) var downloadedURL: URL?
    @Published public private(set) var downloadedBytes: Int64 = 0
    @Published public private(set) var totalBytes: Int64?

    private enum Operation {
        case check, install, download
    }

    private var operation: Operation = .check
    private var transferID: UUID?

    public var isChecking: Bool { phase == .checking }
    public var isDownloading: Bool { [.downloading, .verifying, .installing, .restarting].contains(phase) }
    public var isBusy: Bool { isChecking || isDownloading }

    public var downloadProgress: Double? {
        guard let totalBytes, totalBytes > 0 else { return nil }
        return min(1, Double(downloadedBytes) / Double(totalBytes))
    }

    public var canInstall: Bool {
        guard let release, Self.checksumAsset(from: release.assets) != nil else { return false }
        return Self.installArchive(from: release.assets) != nil
            && Self.isWritableAppBundle(bundleURL: Bundle.main.bundleURL)
    }

    public var canDownload: Bool {
        guard let release, Self.checksumAsset(from: release.assets) != nil else { return false }
        return Self.preferredMacAsset(from: release.assets) != nil
    }

    private init() {}

    public static func currentAppVersion() -> String {
        let ver = MSKeyBrand.version.trimmingCharacters(in: .whitespacesAndNewlines)
        if ver.isEmpty || ver == "development" {
            return "0.2.11"
        }
        return ver
    }

    nonisolated public static func normalizeVersion(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("v") || trimmed.hasPrefix("V") {
            return String(trimmed.dropFirst())
        }
        return trimmed
    }

    nonisolated public static func parseReleaseJSON(_ json: String) -> GitHubReleaseInfo? {
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        if let draft = obj["draft"] as? Bool, draft {
            return nil
        }
        guard let rawTag = obj["tag_name"] as? String else { return nil }
        let tagName = rawTag.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !tagName.isEmpty else { return nil }
        let version = normalizeVersion(tagName)
        let htmlURL = (obj["html_url"] as? String) ?? releasesPageURL
        let body = (obj["body"] as? String) ?? ""

        var assets: [GitHubReleaseAsset] = []
        if let arr = obj["assets"] as? [[String: Any]] {
            for item in arr {
                guard let name = item["name"] as? String,
                      let url = item["browser_download_url"] as? String else {
                    continue
                }
                let size = (item["size"] as? NSNumber)?.uint64Value ?? 0
                assets.append(GitHubReleaseAsset(name: name, downloadURL: url, size: size))
            }
        }
        return GitHubReleaseInfo(
            tagName: tagName,
            version: version,
            htmlURL: htmlURL,
            body: body,
            assets: assets
        )
    }

    nonisolated public static func parseSHA256Sums(_ content: String) -> [String: String] {
        var result: [String: String] = [:]
        content.enumerateLines { line, _ in
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, !trimmed.hasPrefix("#") else { return }
            let parts = trimmed.split(maxSplits: 1, omittingEmptySubsequences: true) { $0.isWhitespace }
            guard parts.count == 2 else { return }
            let hash = String(parts[0]).trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            guard hash.count == 64, hash.allSatisfy({ $0.isHexDigit }) else { return }
            var file = String(parts[1]).trimmingCharacters(in: .whitespacesAndNewlines)
            while file.hasPrefix("*") {
                file.removeFirst()
            }
            while file.hasPrefix("./") {
                file.removeFirst(2)
            }
            if !file.isEmpty {
                result[file] = hash
            }
        }
        return result
    }

    nonisolated public static func sha256Hex(of data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    nonisolated public static func sha256Hex(fileURL: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: fileURL)
        defer { try? handle.close() }
        var hasher = SHA256()
        while autoreleasepool(invoking: {
            let chunk = handle.readData(ofLength: 65536)
            if chunk.isEmpty {
                return false
            }
            hasher.update(data: chunk)
            return true
        }) {}
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    nonisolated public static func verifySHA256(sums: [String: String], filename: String, actualHex: String) -> Bool {
        let cleanActual = actualHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard cleanActual.count == 64 else { return false }
        var cleanName = filename.trimmingCharacters(in: .whitespacesAndNewlines)
        while cleanName.hasPrefix("./") {
            cleanName.removeFirst(2)
        }
        if let expected = sums[cleanName] {
            return expected == cleanActual
        }
        let baseName = URL(fileURLWithPath: cleanName).lastPathComponent
        return sums.contains { key, val in
            URL(fileURLWithPath: key).lastPathComponent == baseName && val == cleanActual
        }
    }

    nonisolated public static func preferredMacAsset(from assets: [GitHubReleaseAsset]) -> GitHubReleaseAsset? {
        preferredMacAsset(from: assets, preferZipForSelfUpdate: false)
    }

    nonisolated public static func preferredMacAsset(
        from assets: [GitHubReleaseAsset],
        preferZipForSelfUpdate: Bool
    ) -> GitHubReleaseAsset? {
        let candidates = assets.filter { asset in
            let lower = asset.name.lowercased()
            let isCli = lower.hasPrefix("mskey-cli-") || lower.contains("-cli-")
            let isMacPkg = lower.hasSuffix(".dmg") || lower.hasSuffix(".pkg")
                || (lower.hasSuffix(".zip") && (lower.contains("mac") || lower.contains("darwin") || lower.contains("osx")))
            return !isCli && isMacPkg
        }
        let exts = preferZipForSelfUpdate ? [".zip", ".dmg", ".pkg"] : [".dmg", ".pkg", ".zip"]
        for ext in exts {
            if let match = candidates.first(where: { $0.name.lowercased().hasSuffix(ext) }) {
                return match
            }
        }
        return nil
    }

    nonisolated public static func isWritableAppBundle(bundleURL: URL) -> Bool {
        guard bundleURL.pathExtension.lowercased() == "app" else { return false }
        let fm = FileManager.default
        let parent = bundleURL.deletingLastPathComponent()
        return fm.isWritableFile(atPath: bundleURL.path) && fm.isWritableFile(atPath: parent.path)
    }

    nonisolated static func installArchive(from assets: [GitHubReleaseAsset]) -> GitHubReleaseAsset? {
        let asset = preferredMacAsset(from: assets, preferZipForSelfUpdate: true)
        return asset?.name.lowercased().hasSuffix(".zip") == true ? asset : nil
    }

    nonisolated public static func checksumAsset(from assets: [GitHubReleaseAsset]) -> GitHubReleaseAsset? {
        assets.first {
            let lower = $0.name.lowercased()
            return lower == "sha256sums" || lower == "sha256sums.txt" || lower.contains("sha256")
        }
    }

    public func checkForUpdates(manual: Bool) {
        if manual { UpdatePanelController.show() }
        guard !isBusy else { return }
        operation = .check
        release = nil
        downloadedURL = nil
        transition(.checking, "Đang kiểm tra cập nhật…")

        Task {
            do {
                let url = URL(string: Self.latestReleaseAPI)!
                let (data, response) = try await Self.metadataSession.data(for: request(url, timeout: 20, isAPI: true))
                try Self.requireSuccess(response)
                guard let release = String(data: data, encoding: .utf8).flatMap(Self.parseReleaseJSON) else {
                    throw UpdateFailure("Dữ liệu phát hành từ GitHub không hợp lệ.")
                }
                let current = Self.currentAppVersion()
                if RustEngine.isNewerVersion(current: current, candidate: release.version) {
                    self.release = release
                    let asset = Self.installArchive(from: release.assets) ?? Self.preferredMacAsset(from: release.assets)
                    let size = asset.flatMap { Int64(exactly: $0.size) }.flatMap { $0 > 0 ? $0 : nil }
                    let summary = size.map { "Gói macOS · \(ByteCountFormatter.string(fromByteCount: $0, countStyle: .file))" }
                    transition(.available, canDownload ? (summary ?? "Gói macOS") : "Mở trang phát hành để tải bản mới.")
                    if !manual { UpdatePanelController.show() }
                } else {
                    transition(.upToDate, "")
                }
            } catch {
                transition(.failed, error.localizedDescription)
            }
        }
    }

    public func installUpdate() {
        guard !isBusy, canInstall, let release, let asset = Self.installArchive(from: release.assets) else { return }
        fetchPackage(asset, release: release, install: true)
    }

    public func downloadUpdate() {
        guard !isBusy, canDownload, let release, let asset = Self.preferredMacAsset(from: release.assets) else { return }
        fetchPackage(asset, release: release, install: false)
    }

    public func retryUpdate() {
        switch operation {
        case .check: checkForUpdates(manual: true)
        case .install: installUpdate()
        case .download: downloadUpdate()
        }
    }

    public func openReleasePage() {
        if let url = URL(string: release?.htmlURL ?? Self.releasesPageURL) {
            NSWorkspace.shared.open(url)
        }
    }

    public func revealDownload() {
        if let downloadedURL { NSWorkspace.shared.activateFileViewerSelecting([downloadedURL]) }
    }

    public func reportRestartFailure(_ message: String) {
        operation = .check
        transition(.failed, message)
        UpdatePanelController.show()
    }

    private func transition(_ phase: UpdatePhase, _ message: String) {
        statusText = message
        self.phase = phase
    }

    private func request(_ url: URL, timeout: TimeInterval, isAPI: Bool = false) -> URLRequest {
        var request = URLRequest(url: url, timeoutInterval: timeout)
        request.setValue("MSKeyMac/\(Self.currentAppVersion())", forHTTPHeaderField: "User-Agent")
        if isAPI { request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept") }
        return request
    }

    private static let metadataSession: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForResource = 60
        return URLSession(configuration: configuration)
    }()

    nonisolated fileprivate static func requireSuccess(_ response: URLResponse?) throws {
        guard let response = response as? HTTPURLResponse else {
            throw UpdateFailure("Máy chủ không trả về phản hồi hợp lệ.")
        }
        guard (200...299).contains(response.statusCode) else {
            throw UpdateFailure("GitHub trả về lỗi HTTP \(response.statusCode).")
        }
    }

    private func fetchPackage(_ asset: GitHubReleaseAsset, release: GitHubReleaseInfo, install: Bool) {
        guard let checksums = Self.checksumAsset(from: release.assets),
              let assetURL = URL(string: asset.downloadURL),
              let checksumURL = URL(string: checksums.downloadURL),
              assetURL.scheme == "https", checksumURL.scheme == "https",
              let filename = Self.safeAssetName(asset.name) else {
            transition(.failed, "Gói cập nhật hoặc tệp kiểm tra SHA-256 không hợp lệ.")
            return
        }
        operation = install ? .install : .download
        downloadedBytes = 0
        totalBytes = asset.size > 0 ? Int64(exactly: asset.size) : nil
        downloadedURL = nil
        transition(.downloading, "Đang tải gói MS Key \(release.version)…")
        let transfer = UUID()
        transferID = transfer

        Task { [self] in
            let fm = FileManager.default
            let archive = fm.temporaryDirectory.appendingPathComponent("mskey-download-\(transfer.uuidString)")
            defer {
                transferID = nil
                try? fm.removeItem(at: archive)
            }
            do {
                let progress = PackageTransfer { [weak self] bytes, total in
                    Task { @MainActor in
                        guard let self, self.transferID == transfer, self.phase == .downloading else { return }
                        self.downloadedBytes = max(self.downloadedBytes, bytes)
                        if total > 0 { self.totalBytes = total }
                    }
                }
                try await progress.download(request(assetURL, timeout: 180), to: archive)
                transition(.verifying, "Đang kiểm tra tính toàn vẹn của gói tải về…")
                let (data, response) = try await Self.metadataSession.data(for: request(checksumURL, timeout: 30))
                try Self.requireSuccess(response)
                let verified = try await Task.detached(priority: .userInitiated) {
                    let digest = try Self.sha256Hex(fileURL: archive)
                    let sums = Self.parseSHA256Sums(String(decoding: data, as: UTF8.self))
                    return Self.verifySHA256(sums: sums, filename: filename, actualHex: digest)
                }.value
                guard verified else { throw UpdateFailure("Mã SHA-256 của gói cập nhật không khớp.") }

                if install {
                    transition(.installing, "Đang chuẩn bị phiên bản mới…")
                    let target = Bundle.main.bundleURL
                    let prepared = try await Task.detached(priority: .userInitiated) {
                        try UpdateHandoff.prepare(archiveURL: archive, target: target, version: release.version)
                    }.value
                    try await Self.startUpdateHelper(prepared)
                    transition(.restarting, "Đang mở lại MS Key sau khi cập nhật…")
                    NSApp.terminate(nil)
                    return
                }

                let downloads = try fm.url(for: .downloadsDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let chosen = downloads.appendingPathComponent(filename)
                let destination = fm.fileExists(atPath: chosen.path)
                    ? downloads.appendingPathComponent("\(UUID().uuidString.prefix(8))-\(filename)") : chosen
                try fm.moveItem(at: archive, to: destination)
                downloadedURL = destination
                transition(.downloaded, "Đã xác minh và lưu gói cập nhật vào Downloads.")
            } catch {
                transition(.failed, error.localizedDescription)
            }
        }
    }

    nonisolated static func safeAssetName(_ name: String) -> String? {
        guard !name.isEmpty, name != ".", name != "..",
              !name.contains("/"), !name.contains("\\"), !name.contains("\0") else { return nil }
        return name
    }

    private static func startUpdateHelper(_ handoff: UpdateHandoff) async throws {
        let process = Process()
        process.executableURL = handoff.helper
        process.arguments = ["--update-helper", handoff.manifest.path]
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            let started = await waitForUpdate(seconds: 75) {
                FileManager.default.fileExists(atPath: handoff.started.path) || !process.isRunning
            }
            guard started, process.isRunning, FileManager.default.fileExists(atPath: handoff.started.path) else {
                if process.isRunning { process.terminate() }
                throw UpdateFailure("Không thể mở trình cập nhật trên máy này. MS Key hiện tại vẫn đang chạy.")
            }
        } catch {
            handoff.cleanup()
            throw error
        }
    }

    public static func runUpdateHelper(manifestPath: String) {
        Task { @MainActor in
            let code = await UpdateHandoff.run(manifestPath: manifestPath)
            exit(code)
        }
        RunLoop.main.run()
    }
}

private struct UpdateFailure: LocalizedError {
    let errorDescription: String?
    init(_ message: String) { errorDescription = message }
}

private final class PackageTransfer: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    private let report: @Sendable (Int64, Int64) -> Void
    private let lock = NSLock()
    private var pending: (URL, CheckedContinuation<Void, Error>)?

    init(_ report: @escaping @Sendable (Int64, Int64) -> Void) {
        self.report = report
    }

    func download(_ request: URLRequest, to destination: URL) async throws {
        try await withCheckedThrowingContinuation { continuation in
            lock.withLock { pending = (destination, continuation) }
            let configuration = URLSessionConfiguration.ephemeral
            configuration.timeoutIntervalForResource = 600
            let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
            session.downloadTask(with: request).resume()
        }
    }

    func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL
    ) {
        guard let (destination, continuation) = takePending() else { return }
        do {
            try UpdateChecker.requireSuccess(downloadTask.response)
            try FileManager.default.moveItem(at: location, to: destination)
            continuation.resume()
        } catch {
            continuation.resume(throwing: error)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let (_, continuation) = takePending() {
            continuation.resume(throwing: error ?? UpdateFailure("Gói tải về không tồn tại."))
        }
        session.finishTasksAndInvalidate()
    }

    func urlSession(
        _ session: URLSession, downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64
    ) {
        report(totalBytesWritten, totalBytesExpectedToWrite)
    }

    private func takePending() -> (URL, CheckedContinuation<Void, Error>)? {
        lock.withLock {
            defer { pending = nil }
            return pending
        }
    }
}

@MainActor
private func waitForUpdate(seconds: Int, until check: () -> Bool) async -> Bool {
    let deadline = ContinuousClock.now.advanced(by: .seconds(seconds))
    while !check() {
        guard ContinuousClock.now < deadline else { return false }
        try? await Task.sleep(for: .milliseconds(120))
    }
    return true
}

struct UpdateHandoff: Codable, Sendable {
    let work: URL
    let target: URL
    let previousPID: Int32
    let version: String

    var manifest: URL { work.appendingPathComponent("handoff.json") }
    var helper: URL { work.appendingPathComponent("MSKeyUpdate") }
    var started: URL { work.appendingPathComponent("helper-started") }
    var ready: URL { work.appendingPathComponent("ready") }
    var failure: URL { work.appendingPathComponent("failure") }
    var stage: URL { target.deletingLastPathComponent().appendingPathComponent(".\(work.lastPathComponent)") }
    var replacement: URL { stage.appendingPathComponent("MSKey.app") }
    var backup: URL { stage.appendingPathComponent("Previous.app") }

    func cleanup(keepBackup: Bool = false) {
        if !keepBackup { try? FileManager.default.removeItem(at: stage) }
        try? FileManager.default.removeItem(at: work)
    }

    static func prepare(archiveURL: URL, target: URL, version: String) throws -> UpdateHandoff {
        let fm = FileManager.default
        let target = target.resolvingSymlinksInPath()
        guard UpdateChecker.isWritableAppBundle(bundleURL: target) else {
            throw UpdateFailure("MS Key cần nằm trong một thư mục Ứng dụng có quyền ghi để cập nhật.")
        }
        _ = try validateBundle(target)
        let handoff = UpdateHandoff(
            work: fm.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("mskey-update-\(UUID().uuidString)"),
            target: target,
            previousPID: ProcessInfo.processInfo.processIdentifier,
            version: version
        )
        try fm.createDirectory(at: handoff.work, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        do {
            try fm.createDirectory(at: handoff.stage, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            try runTool("/usr/bin/ditto", ["-xk", archiveURL.path, handoff.stage.path], "Không giải nén được gói cập nhật.")
            let stagedVersion = try validateBundle(handoff.replacement)
            guard stagedVersion == version else {
                throw UpdateFailure("Phiên bản trong gói cập nhật không khớp với bản phát hành.")
            }
            _ = try? runTool("/usr/bin/xattr", ["-dr", "com.apple.quarantine", handoff.replacement.path], "")
            let signer = handoff.replacement.appendingPathComponent("Contents/Resources/macos-local-sign.sh")
            guard fm.fileExists(atPath: signer.path) else { throw UpdateFailure("Gói cập nhật thiếu công cụ ký cục bộ.") }
            try runTool("/bin/bash", [signer.path, handoff.replacement.path],
                        "Không thể ký bản mới bằng chứng chỉ MS Key của máy này.")
            _ = try validateBundle(handoff.replacement)
            let currentBinary = target.appendingPathComponent("Contents/MacOS/MSKey")
            try fm.copyItem(at: currentBinary, to: handoff.helper)
            try JSONEncoder().encode(handoff).write(to: handoff.manifest, options: .atomic)
            return handoff
        } catch {
            handoff.cleanup()
            throw error
        }
    }

    private static func load(manifestPath: String) throws -> UpdateHandoff {
        let fm = FileManager.default
        let manifest = URL(fileURLWithPath: manifestPath).resolvingSymlinksInPath()
        let work = manifest.deletingLastPathComponent()
        let prefix = "mskey-update-"
        let root = fm.temporaryDirectory.resolvingSymlinksInPath()
        guard manifest.lastPathComponent == "handoff.json",
              work.deletingLastPathComponent().path == root.path,
              work.lastPathComponent.hasPrefix(prefix),
              UUID(uuidString: String(work.lastPathComponent.dropFirst(prefix.count))) != nil else {
            throw UpdateFailure("Đường dẫn bàn giao cập nhật không hợp lệ.")
        }
        let handoff = try JSONDecoder().decode(UpdateHandoff.self, from: Data(contentsOf: manifest))
        guard handoff.work.path == work.path, handoff.previousPID > 0,
              handoff.helper.path == URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath().path,
              handoff.target.path == handoff.target.resolvingSymlinksInPath().path,
              handoff.stage.path == handoff.stage.resolvingSymlinksInPath().path else {
            throw UpdateFailure("Thông tin bàn giao cập nhật không hợp lệ.")
        }
        _ = try validateBundle(handoff.target)
        guard try validateBundle(handoff.replacement) == handoff.version else {
            throw UpdateFailure("Không nhận ra ứng dụng thay thế.")
        }
        return handoff
    }

    @MainActor
    static func run(manifestPath: String) async -> Int32 {
        guard let handoff = try? load(manifestPath: manifestPath) else { return 1 }
        let fm = FileManager.default
        do {
            try Data().write(to: handoff.started, options: .atomic)
        } catch {
            return 1
        }
        let oldExited = await waitForUpdate(seconds: 25) {
            kill(handoff.previousPID, 0) != 0 && errno == ESRCH
        }
        guard oldExited else {
            handoff.cleanup()
            showError("MS Key chưa thoát để hoàn tất cập nhật. Hãy đóng ứng dụng và thử lại.")
            return 1
        }

        var previousMoved = false
        var nextApp: NSRunningApplication?
        do {
            try fm.moveItem(at: handoff.target, to: handoff.backup)
            previousMoved = true
            try fm.moveItem(at: handoff.replacement, to: handoff.target)
            guard try validateBundle(handoff.target) == handoff.version else {
                throw UpdateFailure("Phiên bản đã cài không khớp với gói cập nhật.")
            }
            let launched = try await open(handoff.target, arguments: ["--show", "--update-ready-file", handoff.ready.path],
                                          readyPID: handoff.readyPID)
            nextApp = launched
            let ready = await waitForUpdate(seconds: 18) {
                launched.isTerminated || handoff.readyPID() == launched.processIdentifier
            }
            if ready && !launched.isTerminated && handoff.readyPID() == launched.processIdentifier {
                try? await Task.sleep(for: .milliseconds(500))
                if !launched.isTerminated {
                    handoff.cleanup()
                    return 0
                }
            }
            throw UpdateFailure("Bản \(handoff.version) đã cài nhưng chưa khởi động hoàn chỉnh.")
        } catch is LaunchTimeout {
            showError("macOS chưa xác nhận đã mở MS Key. Bản trước được giữ tại \(handoff.backup.path).")
            return 1
        } catch {
            let originalError = error.localizedDescription
            do {
                if let nextApp, !nextApp.isTerminated {
                    _ = nextApp.terminate()
                    let quit = await waitForUpdate(seconds: 3) { nextApp.isTerminated }
                    if !quit {
                        _ = nextApp.forceTerminate()
                        guard await waitForUpdate(seconds: 3, until: { nextApp.isTerminated }) else {
                            throw UpdateFailure("Không thể thoát bản cập nhật để khôi phục.")
                        }
                    }
                }
                if previousMoved {
                    if fm.fileExists(atPath: handoff.target.path) {
                        let rejected = handoff.stage.appendingPathComponent("Failed.app")
                        try fm.moveItem(at: handoff.target, to: rejected)
                    }
                    try fm.moveItem(at: handoff.backup, to: handoff.target)
                }
                let message = "\(originalError) Đã khôi phục bản MS Key trước."
                try message.write(to: handoff.failure, atomically: true, encoding: .utf8)
                _ = try await open(handoff.target, arguments: ["--show", "--update-failure-file", handoff.failure.path])
                try? fm.removeItem(at: handoff.stage)
                if await waitForUpdate(seconds: 8, until: { !fm.fileExists(atPath: handoff.failure.path) }) {
                    handoff.cleanup()
                }
                return 1
            } catch {
                handoff.cleanup(keepBackup: previousMoved)
                let detail = fm.fileExists(atPath: handoff.backup.path) ? " Bản trước còn ở \(handoff.backup.path)." : ""
                showError("\(originalError) \(error.localizedDescription)\(detail)")
                return 1
            }
        }
    }

    private func readyPID() -> Int32? {
        guard let text = try? String(contentsOf: ready, encoding: .utf8) else { return nil }
        return Int32(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    private struct LaunchTimeout: LocalizedError {
        var errorDescription: String? { "macOS chưa xác nhận đã mở MS Key trong thời gian chờ." }
    }

    @MainActor
    private static func open(_ url: URL, arguments: [String], readyPID: (() -> Int32?)? = nil) async throws -> NSRunningApplication {
        let config = NSWorkspace.OpenConfiguration()
        config.arguments = arguments
        config.activates = true
        config.createsNewApplicationInstance = true
        config.allowsRunningApplicationSubstitution = false
        config.addsToRecentItems = false
        config.promptsUserIfNeeded = false
        var result: Result<NSRunningApplication, Error>?
        Task { @MainActor in
            do {
                result = .success(try await NSWorkspace.shared.openApplication(at: url, configuration: config))
            } catch {
                result = .failure(error)
            }
        }
        _ = await waitForUpdate(seconds: 25) {
            if result == nil, let pid = readyPID?(), let app = NSRunningApplication(processIdentifier: pid),
               !app.isTerminated, app.bundleURL?.resolvingSymlinksInPath().path == url.path {
                result = .success(app)
            }
            return result != nil
        }
        guard let result else { throw LaunchTimeout() }
        return try result.get()
    }

    @MainActor
    private static func showError(_ detail: String) {
        NSApplication.shared.setActivationPolicy(.accessory)
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Không thể hoàn tất cập nhật MS Key"
        alert.informativeText = detail
        alert.addButton(withTitle: "Đóng")
        alert.runModal()
    }

    private static func validateBundle(_ url: URL) throws -> String {
        let fm = FileManager.default
        let executable = url.appendingPathComponent("Contents/MacOS/MSKey")
        let info = url.appendingPathComponent("Contents/Info.plist")
        guard url.path == url.resolvingSymlinksInPath().path,
              let data = try? Data(contentsOf: info),
              let plist = try? PropertyListSerialization.propertyList(from: data, options: [], format: nil) as? [String: Any],
              plist["CFBundleIdentifier"] as? String == "org.mskey.mac",
              plist["CFBundlePackageType"] as? String == "APPL",
              plist["CFBundleExecutable"] as? String == "MSKey",
              let version = plist["CFBundleShortVersionString"] as? String,
              fm.isExecutableFile(atPath: executable.path) else {
            throw UpdateFailure("Gói tải về không chứa ứng dụng MS Key hợp lệ.")
        }
        try runTool("/usr/bin/codesign", ["--verify", "--deep", "--strict", url.path],
                    "Chữ ký ứng dụng MS Key không hợp lệ.", timeout: 30)
        return version
    }

    static func runTool(_ path: String, _ args: [String], _ message: String, timeout: TimeInterval = 90) throws {
        let process = Process()
        let exited = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in exited.signal() }
        process.executableURL = URL(fileURLWithPath: path)
        process.arguments = args
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        if exited.wait(timeout: .now() + timeout) == .timedOut {
            let pid = process.processIdentifier
            if process.isRunning {
                let target = getpgid(pid) == pid ? -pid : pid
                kill(target, SIGKILL)
                _ = exited.wait(timeout: .now() + 2)
            }
            throw UpdateFailure("\(URL(fileURLWithPath: path).lastPathComponent) đã hết thời gian chờ.")
        }
        guard process.terminationStatus == 0 else { throw UpdateFailure(message) }
    }
}
