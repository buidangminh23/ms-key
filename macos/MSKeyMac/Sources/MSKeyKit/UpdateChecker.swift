import AppKit
import Combine
import CryptoKit
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

@MainActor
public final class UpdateChecker: ObservableObject {
    public static let shared = UpdateChecker()
    nonisolated public static let latestReleaseAPI = "https://api.github.com/repos/buidangminh23/ms-key/releases/latest"
    nonisolated public static let releasesPageURL = "https://github.com/buidangminh23/ms-key/releases"

    @Published public private(set) var isChecking = false
    @Published public private(set) var isDownloading = false
    @Published public private(set) var statusText = ""

    private init() {}

    public static func currentAppVersion() -> String {
        let ver = MSKeyBrand.version.trimmingCharacters(in: .whitespacesAndNewlines)
        if ver.isEmpty || ver == "development" {
            return "0.1.5"
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

    nonisolated public static func shellSingleQuote(_ value: String) -> String {
        "'" + value.replacingOccurrences(of: "'", with: "'\"'\"'") + "'"
    }

    nonisolated public static func buildSelfReplaceShellScript(
        pid: Int32,
        stagedAppPath: String,
        targetAppPath: String,
        workDirPath: String
    ) -> String {
        let staged = shellSingleQuote(stagedAppPath)
        let target = shellSingleQuote(targetAppPath)
        let backup = shellSingleQuote(targetAppPath + ".bak")
        let work = shellSingleQuote(workDirPath)
        return """
        i=0
        while kill -0 \(pid) 2>/dev/null && [ $i -lt 120 ]; do
          sleep 0.2
          i=$((i+1))
        done
        xattr -dr com.apple.quarantine \(staged) >/dev/null 2>&1 || true
        rm -rf \(backup)
        if mv \(target) \(backup); then
          if mv \(staged) \(target); then
            rm -rf \(backup)
          else
            mv \(backup) \(target) >/dev/null 2>&1 || true
            rm -rf \(work)
            exit 1
          fi
        else
          rm -rf \(work)
          exit 1
        fi
        rm -rf \(work)
        open -n \(target) --args --show >/dev/null 2>&1 || open -n \(target) >/dev/null 2>&1 || true
        """
    }

    nonisolated public static func checksumAsset(from assets: [GitHubReleaseAsset]) -> GitHubReleaseAsset? {
        assets.first {
            let lower = $0.name.lowercased()
            return lower == "sha256sums" || lower == "sha256sums.txt" || lower.contains("sha256")
        }
    }

    public func checkForUpdates(manual: Bool) {
        guard !isChecking, !isDownloading else { return }
        guard let url = URL(string: Self.latestReleaseAPI) else { return }
        isChecking = true
        statusText = "Đang kiểm tra cập nhật..."

        Task {
            defer { self.isChecking = false }
            let current = Self.currentAppVersion()
            do {
                var req = URLRequest(url: url, timeoutInterval: 15)
                req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
                req.setValue("MSKeyMac/\(current)", forHTTPHeaderField: "User-Agent")
                let (data, response) = try await URLSession.shared.data(for: req)
                if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                    statusText = "Lỗi HTTP \(http.statusCode)"
                    if manual {
                        presentInfoAlert(
                            title: "Không thể kiểm tra cập nhật",
                            message: "GitHub phản hồi mã lỗi HTTP \(http.statusCode)."
                        )
                    }
                    return
                }
                guard let json = String(data: data, encoding: .utf8),
                      let release = Self.parseReleaseJSON(json) else {
                    statusText = "Dữ liệu phát hành không hợp lệ"
                    if manual {
                        presentInfoAlert(
                            title: "Không thể đọc thông tin phát hành",
                            message: "Dữ liệu phát hành từ GitHub không hợp lệ."
                        )
                    }
                    return
                }

                if RustEngine.isNewerVersion(current: current, candidate: release.version) {
                    statusText = "Đã có phiên bản mới \(release.version)"
                    presentUpdateAvailableAlert(release: release, current: current)
                } else {
                    statusText = "Đang dùng bản mới nhất (\(current))"
                    if manual {
                        presentInfoAlert(
                            title: "Bạn đang dùng bản mới nhất",
                            message: "MS Key phiên bản \(current) là phiên bản mới nhất hiện có."
                        )
                    }
                }
            } catch {
                statusText = "Lỗi kiểm tra cập nhật"
                if manual {
                    presentInfoAlert(
                        title: "Lỗi kết nối kiểm tra cập nhật",
                        message: error.localizedDescription
                    )
                }
            }
        }
    }

    private func presentUpdateAvailableAlert(release: GitHubReleaseInfo, current: String) {
        let canSelfInstall = Self.isWritableAppBundle(bundleURL: Bundle.main.bundleURL)
        let installAsset = canSelfInstall
            ? Self.preferredMacAsset(from: release.assets, preferZipForSelfUpdate: true)
            : nil
        let downloadAsset = Self.preferredMacAsset(from: release.assets, preferZipForSelfUpdate: false)
        let shaAsset = Self.checksumAsset(from: release.assets)

        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Có bản cập nhật mới: MS Key \(release.version)"
        let notes = release.body.trimmingCharacters(in: .whitespacesAndNewlines)
        let clippedNotes = notes.count > 420 ? String(notes.prefix(420)) + "…" : notes
        var info = "Phiên bản hiện tại: \(current)\nPhiên bản mới: \(release.version)"
        let previewAsset = installAsset ?? downloadAsset
        if let previewAsset {
            let sizeMB = Double(previewAsset.size) / 1_048_576.0
            let sizeText = previewAsset.size > 0 ? String(format: " (%.1f MB)", sizeMB) : ""
            info += "\nGói macOS: \(previewAsset.name)\(sizeText)"
            if let shaAsset {
                info += "\nSẽ xác minh mã SHA-256 từ \(shaAsset.name) sau khi tải."
            }
        }
        if !clippedNotes.isEmpty {
            info += "\n\nGhi chú phát hành:\n\(clippedNotes)"
        }
        alert.informativeText = info

        var actions: [() -> Void] = []
        if let installAsset {
            alert.addButton(withTitle: "Cập nhật & Khởi động lại")
            actions.append { [self] in
                downloadAndVerifyAsset(installAsset, shaAsset: shaAsset, releaseURL: release.htmlURL, autoInstall: true)
            }
        }
        if let downloadAsset {
            alert.addButton(withTitle: "Tải về Downloads")
            actions.append { [self] in
                downloadAndVerifyAsset(downloadAsset, shaAsset: shaAsset, releaseURL: release.htmlURL, autoInstall: false)
            }
        }
        alert.addButton(withTitle: "Mở trang phát hành")
        actions.append { [self] in
            openURL(release.htmlURL)
        }
        alert.addButton(withTitle: "Để sau")

        NSApp.activate(ignoringOtherApps: true)
        let resp = alert.runModal()
        let idx = resp.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        if idx >= 0 && idx < actions.count {
            actions[idx]()
        }
    }

    private func downloadAndVerifyAsset(
        _ asset: GitHubReleaseAsset,
        shaAsset: GitHubReleaseAsset?,
        releaseURL: String,
        autoInstall: Bool
    ) {
        guard let assetURL = URL(string: asset.downloadURL) else {
            openURL(releaseURL)
            return
        }
        isDownloading = true
        statusText = "Đang tải \(asset.name)..."
        Task {
            defer { self.isDownloading = false }
            do {
                let current = Self.currentAppVersion()
                var req = URLRequest(url: assetURL, timeoutInterval: 180)
                req.setValue("MSKeyMac/\(current)", forHTTPHeaderField: "User-Agent")
                let (tempURL, resp) = try await URLSession.shared.download(for: req)
                if let http = resp as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                    try? FileManager.default.removeItem(at: tempURL)
                    statusText = "Tải về thất bại (HTTP \(http.statusCode))"
                    presentInfoAlert(
                        title: "Tải gói cập nhật thất bại",
                        message: "Máy chủ phản hồi mã HTTP \(http.statusCode)."
                    )
                    return
                }
                statusText = "Đang xác minh SHA-256..."
                let actualDigest = try await Task.detached(priority: .userInitiated) {
                    try UpdateChecker.sha256Hex(fileURL: tempURL)
                }.value

                var checksumVerifiedNote = "Không có tệp SHA256SUMS kèm bản phát hành."
                if let shaAsset, let shaURL = URL(string: shaAsset.downloadURL) {
                    var shaReq = URLRequest(url: shaURL, timeoutInterval: 30)
                    shaReq.setValue("MSKeyMac/\(current)", forHTTPHeaderField: "User-Agent")
                    let (shaData, shaResp) = try await URLSession.shared.data(for: shaReq)
                    if let http = shaResp as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                        try? FileManager.default.removeItem(at: tempURL)
                        statusText = "Không tải được SHA256SUMS"
                        presentInfoAlert(
                            title: "Không tải được SHA256SUMS",
                            message: "Không thể xác minh toàn vẹn gói tải về (HTTP \(http.statusCode))."
                        )
                        return
                    }
                    let sumText = String(data: shaData, encoding: .utf8) ?? ""
                    let sums = Self.parseSHA256Sums(sumText)
                    guard Self.verifySHA256(sums: sums, filename: asset.name, actualHex: actualDigest) else {
                        try? FileManager.default.removeItem(at: tempURL)
                        statusText = "Xác minh SHA-256 thất bại"
                        presentInfoAlert(
                            title: "Xác minh SHA-256 thất bại",
                            message: "Mã SHA-256 của \(asset.name) không khớp với \(shaAsset.name). Tệp tải về đã bị hủy để đảm bảo an toàn."
                        )
                        return
                    }
                    checksumVerifiedNote = "Đã xác minh SHA-256 khớp với \(shaAsset.name):\n\(actualDigest)"
                }

                if autoInstall {
                    do {
                        statusText = "Đang cài đặt bản mới..."
                        let bundleURL = Bundle.main.bundleURL
                        try await Task.detached(priority: .userInitiated) {
                            try Self.installDownloadedArchiveAndScheduleRelaunch(
                                archiveURL: tempURL,
                                assetName: asset.name,
                                targetAppURL: bundleURL
                            )
                        }.value
                        statusText = "Đang khởi động lại MS Key..."
                        NSApp.terminate(nil)
                        return
                    } catch {
                        try? FileManager.default.removeItem(at: tempURL)
                        statusText = "Lỗi tự động cập nhật"
                        presentInfoAlert(
                            title: "Không thể tự động cập nhật",
                            message: "\(error.localizedDescription)\n\nBạn có thể tải bản mới về Downloads hoặc mở GitHub."
                        )
                        return
                    }
                }

                let downloadsDir = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
                    ?? FileManager.default.temporaryDirectory
                var destURL = downloadsDir.appendingPathComponent(asset.name)
                if FileManager.default.fileExists(atPath: destURL.path) {
                    let stem = destURL.deletingPathExtension().lastPathComponent
                    let ext = destURL.pathExtension
                    let stamp = Int(Date().timeIntervalSince1970)
                    let uniqueName = ext.isEmpty ? "\(stem)-\(stamp)" : "\(stem)-\(stamp).\(ext)"
                    destURL = downloadsDir.appendingPathComponent(uniqueName)
                }
                try FileManager.default.moveItem(at: tempURL, to: destURL)
                statusText = "Đã lưu tại Downloads"
                NSWorkspace.shared.activateFileViewerSelecting([destURL])
                presentInfoAlert(
                    title: "Tải về hoàn tất",
                    message: "Đã lưu tại: \(destURL.path)\n\(checksumVerifiedNote)"
                )
            } catch {
                statusText = "Lỗi tải cập nhật"
                presentInfoAlert(
                    title: "Lỗi tải cập nhật",
                    message: error.localizedDescription
                )
            }
        }
    }

    nonisolated private static func installDownloadedArchiveAndScheduleRelaunch(
        archiveURL: URL,
        assetName: String,
        targetAppURL: URL
    ) throws {
        let fm = FileManager.default
        let workDir = fm.temporaryDirectory.appendingPathComponent("mskey-selfupdate-\(ProcessInfo.processInfo.processIdentifier)")
        try? fm.removeItem(at: workDir)
        try fm.createDirectory(at: workDir, withIntermediateDirectories: true)

        let stagedAppURL = workDir.appendingPathComponent("MSKey.app")
        let lower = assetName.lowercased()
        if lower.hasSuffix(".zip") {
            let extractDir = workDir.appendingPathComponent("zip")
            try fm.createDirectory(at: extractDir, withIntermediateDirectories: true)
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
            p.arguments = ["-xk", archiveURL.path, extractDir.path]
            try p.run()
            p.waitUntilExit()
            guard p.terminationStatus == 0 else {
                throw NSError(domain: "MSKeyUpdate", code: 1, userInfo: [NSLocalizedDescriptionKey: "Giải nén gói .zip thất bại."])
            }
            guard let foundApp = findDotApp(in: extractDir) else {
                throw NSError(domain: "MSKeyUpdate", code: 2, userInfo: [NSLocalizedDescriptionKey: "Không tìm thấy MSKey.app trong gói cập nhật."])
            }
            try fm.moveItem(at: foundApp, to: stagedAppURL)
        } else if lower.hasSuffix(".dmg") {
            let mountDir = workDir.appendingPathComponent("mnt")
            try fm.createDirectory(at: mountDir, withIntermediateDirectories: true)
            let attach = Process()
            attach.executableURL = URL(fileURLWithPath: "/usr/bin/hdiutil")
            attach.arguments = ["attach", archiveURL.path, "-nobrowse", "-readonly", "-mountpoint", mountDir.path]
            try attach.run()
            attach.waitUntilExit()
            guard attach.terminationStatus == 0 else {
                throw NSError(domain: "MSKeyUpdate", code: 3, userInfo: [NSLocalizedDescriptionKey: "Không mount được file .dmg."])
            }
            defer {
                let detach = Process()
                detach.executableURL = URL(fileURLWithPath: "/usr/bin/hdiutil")
                detach.arguments = ["detach", mountDir.path, "-force"]
                try? detach.run()
                detach.waitUntilExit()
            }
            guard let foundApp = findDotApp(in: mountDir) else {
                throw NSError(domain: "MSKeyUpdate", code: 4, userInfo: [NSLocalizedDescriptionKey: "Không tìm thấy MSKey.app trong .dmg."])
            }
            let copyProc = Process()
            copyProc.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
            copyProc.arguments = [foundApp.path, stagedAppURL.path]
            try copyProc.run()
            copyProc.waitUntilExit()
            guard copyProc.terminationStatus == 0 else {
                throw NSError(domain: "MSKeyUpdate", code: 5, userInfo: [NSLocalizedDescriptionKey: "Không sao chép được MSKey.app từ .dmg."])
            }
        } else {
            throw NSError(domain: "MSKeyUpdate", code: 6, userInfo: [NSLocalizedDescriptionKey: "Định dạng \(assetName) không hỗ trợ cập nhật trực tiếp trong app."])
        }

        try? fm.removeItem(at: archiveURL)
        let script = buildSelfReplaceShellScript(
            pid: ProcessInfo.processInfo.processIdentifier,
            stagedAppPath: stagedAppURL.path,
            targetAppPath: targetAppURL.path,
            workDirPath: workDir.path
        )
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/sh")
        proc.arguments = ["-c", script]
        try proc.run()
    }

    nonisolated private static func findDotApp(in directory: URL) -> URL? {
        guard let enumerator = FileManager.default.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return nil
        }
        for case let url as URL in enumerator {
            if url.pathExtension.lowercased() == "app" {
                return url
            }
        }
        return nil
    }

    private func openURL(_ raw: String) {
        let target = URL(string: raw) ?? URL(string: Self.releasesPageURL)
        if let target {
            NSWorkspace.shared.open(target)
        }
    }

    private func presentInfoAlert(title: String, message: String) {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "Đóng")
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }
}
