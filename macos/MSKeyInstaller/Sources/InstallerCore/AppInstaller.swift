import AppKit
import Darwin
import Foundation

public enum InstallPhase: Sendable, Equatable {
    case preparing, copying, verifying, finishing
}

public struct InstallProgress: Sendable {
    public let phase: InstallPhase
    public let completedBytes: Int64
    public let totalBytes: Int64
}

public enum InstallError: LocalizedError {
    case cancelled
    case invalidApp(String)
    case unsafePath(String)
    case alreadyInstalling
    case appRunning
    case system(String, Int32)
    case rollbackFailed(String)

    public var errorDescription: String? {
        switch self {
        case .cancelled: return "Đã hủy cài đặt."
        case .invalidApp(let detail): return "Không thể xác minh ứng dụng MS Key: \(detail)"
        case .unsafePath(let detail): return "Đường dẫn cài đặt không an toàn: \(detail)"
        case .alreadyInstalling: return "Một trình cài đặt MS Key khác đang chạy."
        case .appRunning: return "Hãy thoát MS Key trước khi cài bản cập nhật này."
        case .system(let operation, let code): return "\(operation): \(String(cString: strerror(code)))"
        case .rollbackFailed(let path): return "Bản MS Key trước được giữ tại \(path). Hãy khôi phục bản này trước khi thử lại."
        }
    }
}

public final class InstallCancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false

    public init() {}

    public func cancel() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }

    public func check() throws {
        lock.lock()
        let value = cancelled
        lock.unlock()
        if value { throw InstallError.cancelled }
    }
}

public struct AppInstaller: Sendable {
    private let validate: @Sendable (URL) throws -> Void
    private let isRunning: @Sendable (URL) -> Bool
    private let move: @Sendable (URL, URL) throws -> Void

    public init() {
        validate = Self.validateSignature
        isRunning = Self.runningApplication
        move = { try FileManager.default.moveItem(at: $0, to: $1) }
    }

    init(
        validate: @escaping @Sendable (URL) throws -> Void,
        isRunning: @escaping @Sendable (URL) -> Bool = { _ in false },
        move: @escaping @Sendable (URL, URL) throws -> Void = { try FileManager.default.moveItem(at: $0, to: $1) }
    ) {
        self.validate = validate
        self.isRunning = isRunning
        self.move = move
    }

    public func install(
        source: URL,
        destinationDirectory: URL,
        cancellation: InstallCancellation,
        onProgress: @escaping @Sendable (InstallProgress) -> Void
    ) throws -> URL {
        let manager = FileManager.default
        try cancellation.check()
        onProgress(InstallProgress(phase: .preparing, completedBytes: 0, totalBytes: 0))
        let originalSource = source.standardizedFileURL
        var sourceInfo = stat()
        guard lstat(originalSource.path, &sourceInfo) == 0,
              sourceInfo.st_mode & S_IFMT == S_IFDIR else {
            throw InstallError.invalidApp("Nguồn cài đặt phải là thư mục ứng dụng, không phải liên kết.")
        }
        let source = originalSource.resolvingSymlinksInPath()
        let directory = destinationDirectory.standardizedFileURL.resolvingSymlinksInPath()
        let target = directory.appendingPathComponent("MSKey.app", isDirectory: true)
        guard !Self.contains(source, target),
              !Self.contains(target, source) else {
            throw InstallError.unsafePath("Nguồn và đích phải nằm ở hai vị trí riêng biệt.")
        }
        var directoryInfo = stat()
        guard lstat(directory.path, &directoryInfo) == 0,
              directoryInfo.st_mode & S_IFMT == S_IFDIR else {
            throw InstallError.unsafePath("Thư mục đích chưa tồn tại.")
        }
        let installLock = try DestinationLock(directory: directory)
        defer { installLock.unlock() }
        try Self.validateIdentity(source)
        let totalBytes = try Self.inspectTree(source, cancellation: cancellation)
        try validate(source)
        try validateTarget(target)
        guard !isRunning(target) else { throw InstallError.appRunning }
        try cancellation.check()

        let stage = directory.appendingPathComponent(".MSKey-install-\(UUID().uuidString).app")
        let backup = directory.appendingPathComponent(".MSKey-backup-\(UUID().uuidString).app")
        defer { try? manager.removeItem(at: stage) }
        let context = CopyContext(totalBytes: totalBytes, cancellation: cancellation, progress: onProgress)
        onProgress(InstallProgress(phase: .copying, completedBytes: 0, totalBytes: totalBytes))
        try copy(source: source, destination: stage, context: context)
        try cancellation.check()
        onProgress(InstallProgress(phase: .verifying, completedBytes: context.completedBytes, totalBytes: totalBytes))
        try Self.validateIdentity(stage)
        let copiedBytes = try Self.inspectTree(stage, cancellation: cancellation)
        guard copiedBytes == totalBytes, context.completedBytes == totalBytes else {
            throw InstallError.invalidApp("Nội dung ứng dụng đã thay đổi khi sao chép. Hãy mở lại trình cài đặt.")
        }
        try validate(stage)
        if Self.applyLocalSigning(stage) {
            try validate(stage)
        }
        try validateTarget(target)
        guard !isRunning(target) else { throw InstallError.appRunning }
        try cancellation.check()
        onProgress(InstallProgress(phase: .finishing, completedBytes: totalBytes, totalBytes: totalBytes))
        try cancellation.check()

        let hadTarget = Self.exists(target)
        if hadTarget { try move(target, backup) }
        do {
            try move(stage, target)
        } catch {
            if hadTarget {
                do { try move(backup, target) }
                catch { throw InstallError.rollbackFailed(backup.path) }
            }
            throw error
        }
        if hadTarget { try? manager.removeItem(at: backup) }
        return target
    }

    private func validateTarget(_ target: URL) throws {
        if Self.exists(target) {
            try Self.validateIdentity(target)
            _ = try Self.inspectTree(target, cancellation: InstallCancellation())
        }
    }

    private static func contains(_ root: URL, _ child: URL) -> Bool {
        child.path == root.path || child.path.hasPrefix(root.path + "/")
    }

    private static func exists(_ url: URL) -> Bool {
        var info = stat()
        return lstat(url.path, &info) == 0
    }

    private static func validateIdentity(_ app: URL) throws {
        var info = stat()
        guard app.pathExtension == "app", lstat(app.path, &info) == 0,
              info.st_mode & S_IFMT == S_IFDIR,
              let data = try? Data(contentsOf: app.appendingPathComponent("Contents/Info.plist")),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any],
              plist["CFBundleIdentifier"] as? String == "org.mskey.mac",
              plist["CFBundlePackageType"] as? String == "APPL",
              plist["CFBundleExecutable"] as? String == "MSKey",
              FileManager.default.isExecutableFile(atPath: app.appendingPathComponent("Contents/MacOS/MSKey").path) else {
            throw InstallError.invalidApp("Bộ cài không chứa ứng dụng MS Key hợp lệ.")
        }
    }

    private static func inspectTree(_ app: URL, cancellation: InstallCancellation) throws -> Int64 {
        var enumerationError: Error?
        guard let enumerator = FileManager.default.enumerator(at: app, includingPropertiesForKeys: nil, errorHandler: { _, error in
            enumerationError = error
            return false
        }) else { throw InstallError.invalidApp("Không thể đọc ứng dụng.") }
        var total: Int64 = 0
        for case let entry as URL in enumerator {
            try cancellation.check()
            var info = stat()
            guard lstat(entry.path, &info) == 0 else { throw InstallError.system("Đọc nội dung ứng dụng", errno) }
            switch info.st_mode & S_IFMT {
            case S_IFREG:
                let result = total.addingReportingOverflow(Int64(info.st_size))
                guard !result.overflow else { throw InstallError.invalidApp("Ứng dụng quá lớn.") }
                total = result.partialValue
            case S_IFDIR: break
            case S_IFLNK:
                let link = try FileManager.default.destinationOfSymbolicLink(atPath: entry.path)
                let resolved = entry.resolvingSymlinksInPath()
                guard !link.hasPrefix("/"), contains(app, resolved), resolved != entry,
                      FileManager.default.fileExists(atPath: resolved.path) else {
                    throw InstallError.unsafePath("Liên kết không hợp lệ trong ứng dụng: \(entry.lastPathComponent)")
                }
            default: throw InstallError.unsafePath("Tệp không được hỗ trợ: \(entry.lastPathComponent)")
            }
        }
        if let enumerationError { throw enumerationError }
        return total
    }

    @discardableResult
    private static func applyLocalSigning(_ app: URL) -> Bool {
        let signer = app.appendingPathComponent("Contents/Resources/macos-local-sign.sh")
        guard FileManager.default.fileExists(atPath: signer.path) else { return false }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = [signer.path, app.path]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return false
        }
        return process.terminationStatus == 0
    }

    private static func validateSignature(_ app: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
        process.arguments = ["--verify", "--deep", "--strict", app.path]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw InstallError.invalidApp("Chữ ký ứng dụng không hợp lệ hoặc bị thiếu.")
        }
    }

    private static func runningApplication(_ target: URL) -> Bool {
        NSRunningApplication.runningApplications(withBundleIdentifier: "org.mskey.mac").contains { app in
            guard let url = app.bundleURL else { return true }
            return url.resolvingSymlinksInPath() == target.resolvingSymlinksInPath()
        }
    }

    private func copy(source: URL, destination: URL, context: CopyContext) throws {
        guard let state = copyfile_state_alloc() else { throw InstallError.system("Chuẩn bị sao chép", ENOMEM) }
        defer { copyfile_state_free(state) }
        let pointer = Unmanaged.passUnretained(context).toOpaque()
        let callback: copyfile_callback_t = { what, stage, state, source, _, opaque in
            guard let opaque else { return COPYFILE_QUIT }
            let context = Unmanaged<CopyContext>.fromOpaque(opaque).takeUnretainedValue()
            do { try context.cancellation.check() }
            catch { return COPYFILE_QUIT }
            if stage == COPYFILE_ERR { return COPYFILE_QUIT }
            if what == COPYFILE_COPY_DATA, stage == COPYFILE_PROGRESS, let state, let source {
                var copied: off_t = 0
                guard copyfile_state_get(state, UInt32(COPYFILE_STATE_COPIED), &copied) == 0 else { return COPYFILE_QUIT }
                context.record(path: String(cString: source), copied: Int64(copied))
            }
            return COPYFILE_CONTINUE
        }
        guard copyfile_state_set(state, UInt32(COPYFILE_STATE_STATUS_CTX), pointer) == 0,
              copyfile_state_set(state, UInt32(COPYFILE_STATE_STATUS_CB), unsafeBitCast(callback, to: UnsafeRawPointer.self)) == 0 else {
            throw InstallError.system("Chuẩn bị tiến trình sao chép", errno)
        }
        let result = copyfile(source.path, destination.path, state, copyfile_flags_t(COPYFILE_ALL | COPYFILE_RECURSIVE | COPYFILE_NOFOLLOW | COPYFILE_EXCL))
        let code = errno
        try context.cancellation.check()
        guard result == 0 else { throw InstallError.system("Sao chép MS Key", code) }
    }
}

private final class CopyContext {
    let totalBytes: Int64
    let cancellation: InstallCancellation
    let progress: @Sendable (InstallProgress) -> Void
    var completedBytes: Int64 = 0
    private var files: [String: Int64] = [:]

    init(totalBytes: Int64, cancellation: InstallCancellation, progress: @escaping @Sendable (InstallProgress) -> Void) {
        self.totalBytes = totalBytes
        self.cancellation = cancellation
        self.progress = progress
    }

    func record(path: String, copied: Int64) {
        let previous = files[path, default: 0]
        guard copied > previous else { return }
        files[path] = copied
        completedBytes += copied - previous
        progress(InstallProgress(phase: .copying, completedBytes: completedBytes, totalBytes: totalBytes))
    }
}

private final class DestinationLock {
    private let descriptor: Int32

    init(directory: URL) throws {
        let path = directory.appendingPathComponent(".mskey-install.lock").path
        let permissions = mode_t(S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH)
        let created = open(path, O_CREAT | O_EXCL | O_RDONLY | O_NOFOLLOW | O_CLOEXEC, permissions)
        if created >= 0 {
            guard fchmod(created, permissions) == 0 else {
                let code = errno
                close(created)
                throw InstallError.system("Chuẩn bị khóa phiên cài đặt", code)
            }
            descriptor = created
        } else if errno == EEXIST {
            descriptor = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)
        } else {
            throw InstallError.system("Tạo khóa phiên cài đặt", errno)
        }
        guard descriptor >= 0 else { throw InstallError.system("Khóa phiên cài đặt", errno) }
        var info = stat()
        guard fstat(descriptor, &info) == 0, info.st_mode & S_IFMT == S_IFREG else {
            close(descriptor)
            throw InstallError.unsafePath("Tệp khóa phiên cài đặt không hợp lệ.")
        }
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
            close(descriptor)
            throw InstallError.alreadyInstalling
        }
    }

    func unlock() {
        flock(descriptor, LOCK_UN)
        close(descriptor)
    }
}
