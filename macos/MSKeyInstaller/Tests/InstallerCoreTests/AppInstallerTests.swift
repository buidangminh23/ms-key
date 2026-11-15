import Darwin
import Foundation
import XCTest
@testable import InstallerCore

final class AppInstallerTests: XCTestCase {
    private var root: URL!
    private var source: URL!
    private var destination: URL!

    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).resolvingSymlinksInPath()
        source = root.appendingPathComponent("Source/MSKey.app")
        destination = root.appendingPathComponent("Applications")
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
        try fixture(at: source, text: "new version")
    }

    override func tearDownWithError() throws {
        try FileManager.default.removeItem(at: root)
    }

    func testFreshInstallHasMonotonicByteProgressAndPreservesMetadata() throws {
        let executable = source.appendingPathComponent("Contents/MacOS/MSKey")
        let attribute = "test quarantine metadata"
        let result = attribute.withCString { setxattr(executable.path, "com.mskey.test", $0, attribute.utf8.count, 0, 0) }
        XCTAssertEqual(result, 0)
        let quarantine = "0083;67500000;MSKeyInstallerTests;"
        XCTAssertEqual(quarantine.withCString { setxattr(source.path, "com.apple.quarantine", $0, quarantine.utf8.count, 0, 0) }, 0)
        let events = EventRecorder()
        let installed = try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { events.record($0) }
        XCTAssertEqual(installed, destination.appendingPathComponent("MSKey.app"))
        XCTAssertEqual(try Data(contentsOf: installed.appendingPathComponent("Contents/MacOS/MSKey")), try Data(contentsOf: executable))
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: installed.appendingPathComponent("Contents/MacOS/MSKey").path))
        let attributeLength = getxattr(installed.appendingPathComponent("Contents/MacOS/MSKey").path, "com.mskey.test", nil, 0, 0, 0)
        XCTAssertEqual(attributeLength, attribute.utf8.count)
        XCTAssertGreaterThan(getxattr(installed.path, "com.apple.quarantine", nil, 0, 0, 0), 0)
        let copying = events.values.filter { $0.phase == .copying }
        XCTAssertGreaterThan(copying.count, 1)
        XCTAssertEqual(copying.first?.completedBytes, 0)
        XCTAssertEqual(copying.last?.completedBytes, copying.last?.totalBytes)
        for pair in zip(copying, copying.dropFirst()) {
            XCTAssertLessThanOrEqual(pair.0.completedBytes, pair.1.completedBytes)
            XCTAssertLessThanOrEqual(pair.1.completedBytes, pair.1.totalBytes)
        }
        XCTAssertEqual(events.values.last?.phase, .finishing)
        try assertNoStagingFiles()
    }

    func testReplacementAndInternalSymlinkPreserved() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        try FileManager.default.createSymbolicLink(atPath: source.appendingPathComponent("Contents/current").path, withDestinationPath: "MacOS/MSKey")
        _ = try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "new version")
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: target.appendingPathComponent("Contents/current").path), "MacOS/MSKey")
        try assertNoStagingFiles()
    }

    func testInvalidSignatureLeavesExistingAppUntouched() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        XCTAssertThrowsError(try AppInstaller().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "old version")
        try assertNoStagingFiles()
    }

    func testStagedVerificationFailureLeavesExistingAppUntouched() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        let service = AppInstaller(validate: { url in
            if url.lastPathComponent.hasPrefix(".MSKey-install-") { throw InstallError.invalidApp("test failure") }
        })
        XCTAssertThrowsError(try service.install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "old version")
        try assertNoStagingFiles()
    }

    func testFailedCommitRestoresPreviousApp() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        let service = AppInstaller(validate: { _ in }, move: { from, to in
            if from.lastPathComponent.hasPrefix(".MSKey-install-") { throw InstallError.system("Injected move failure", EIO) }
            try FileManager.default.moveItem(at: from, to: to)
        })
        XCTAssertThrowsError(try service.install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "old version")
        try assertNoStagingFiles()
    }

    func testCancellationDuringCopyPreservesPreviousApp() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        try Data(repeating: 7, count: 2_000_000).write(to: source.appendingPathComponent("Contents/payload"))
        let cancellation = InstallCancellation()
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: cancellation) { progress in
            if progress.phase == .copying && progress.completedBytes > 0 { cancellation.cancel() }
        }) { error in
            guard case InstallError.cancelled = error else { return XCTFail("Unexpected error: \(error)") }
        }
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "old version")
        try assertNoStagingFiles()
    }

    func testEscapingAndAbsoluteLinksAreRejected() throws {
        let link = source.appendingPathComponent("Contents/escape")
        for path in ["../../../../etc/passwd", "/etc/passwd"] {
            try FileManager.default.createSymbolicLink(atPath: link.path, withDestinationPath: path)
            XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
            try FileManager.default.removeItem(at: link)
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: destination.appendingPathComponent("MSKey.app").path))
    }

    func testSameAndNestedLocationsAreRejected() throws {
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: source.deletingLastPathComponent(), cancellation: InstallCancellation()) { _ in })
        let nested = source.appendingPathComponent("Contents/Applications")
        try FileManager.default.createDirectory(at: nested, withIntermediateDirectories: true)
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: nested, cancellation: InstallCancellation()) { _ in })
    }

    func testUnrelatedDestinationIsNeverReplaced() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "unrelated")
        try Data("invalid plist".utf8).write(to: target.appendingPathComponent("Contents/Info.plist"))
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        XCTAssertEqual(try String(contentsOf: target.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "unrelated")
    }

    func testSourceAndDestinationRootSymlinksAreRejected() throws {
        let sourceLink = root.appendingPathComponent("Link.app")
        try FileManager.default.createSymbolicLink(at: sourceLink, withDestinationURL: source)
        XCTAssertThrowsError(try installer().install(source: sourceLink, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        let target = destination.appendingPathComponent("MSKey.app")
        try FileManager.default.createSymbolicLink(at: target, withDestinationURL: source)
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in })
        XCTAssertEqual(try String(contentsOf: source.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "new version")
    }

    func testCancellationBeforePreparingDoesNotCreateDestination() throws {
        let cancellation = InstallCancellation()
        cancellation.cancel()
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: cancellation) { _ in })
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: destination.path), [])
    }

    func testFailedRollbackPreservesRecoverableBackup() throws {
        let target = destination.appendingPathComponent("MSKey.app")
        try fixture(at: target, text: "old version")
        let service = AppInstaller(validate: { _ in }, move: { from, to in
            if from.lastPathComponent.hasPrefix(".MSKey-") { throw InstallError.system("Injected move failure", EIO) }
            try FileManager.default.moveItem(at: from, to: to)
        })
        XCTAssertThrowsError(try service.install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }) { error in
            guard case InstallError.rollbackFailed(let path) = error else { return XCTFail("Unexpected error: \(error)") }
            XCTAssertEqual(try? String(contentsOf: URL(fileURLWithPath: path).appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "old version")
        }
    }

    func testRunningAppDetectedAgainBeforeCommit() throws {
        let calls = Counter()
        let service = AppInstaller(validate: { _ in }, isRunning: { _ in calls.next() >= 2 })
        XCTAssertThrowsError(try service.install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }) { error in
            guard case InstallError.appRunning = error else { return XCTFail("Unexpected error: \(error)") }
        }
        XCTAssertEqual(calls.value, 2)
        XCTAssertFalse(FileManager.default.fileExists(atPath: destination.appendingPathComponent("MSKey.app").path))
        try assertNoStagingFiles()
    }

    func testConcurrentInstallerIsRejected() throws {
        let lockPath = destination.appendingPathComponent(".mskey-install.lock").path
        let descriptor = open(lockPath, O_CREAT | O_RDONLY, S_IRUSR | S_IRGRP | S_IROTH)
        XCTAssertGreaterThanOrEqual(descriptor, 0)
        defer { flock(descriptor, LOCK_UN); close(descriptor) }
        XCTAssertEqual(flock(descriptor, LOCK_EX | LOCK_NB), 0)
        XCTAssertThrowsError(try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }) { error in
            guard case InstallError.alreadyInstalling = error else { return XCTFail("Unexpected error: \(error)") }
        }
    }

    func testExistingReadOnlyLockCanBeReusedWithoutReplacingItsInode() throws {
        let lockPath = destination.appendingPathComponent(".mskey-install.lock").path
        let descriptor = open(lockPath, O_CREAT | O_EXCL | O_RDONLY, S_IRUSR | S_IRGRP | S_IROTH)
        XCTAssertGreaterThanOrEqual(descriptor, 0)
        defer { close(descriptor) }
        var original = stat()
        XCTAssertEqual(fstat(descriptor, &original), 0)
        _ = try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }
        var current = stat()
        XCTAssertEqual(lstat(lockPath, &current), 0)
        XCTAssertEqual(current.st_ino, original.st_ino)
        XCTAssertEqual(current.st_mode & 0o777, 0o444)
    }

    func testNewLockRemainsReadableAcrossUsersDespiteRestrictiveUmask() throws {
        let previousMask = umask(0o077)
        defer { umask(previousMask) }
        _ = try installer().install(source: source, destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }
        var info = stat()
        XCTAssertEqual(lstat(destination.appendingPathComponent(".mskey-install.lock").path, &info), 0)
        XCTAssertEqual(info.st_mode & 0o777, 0o644)
    }

    func testParentDirectorySymlinkIsSupported() throws {
        let alias = root.appendingPathComponent("Alias")
        try FileManager.default.createSymbolicLink(at: alias, withDestinationURL: source.deletingLastPathComponent())
        let installed = try installer().install(source: alias.appendingPathComponent("MSKey.app"), destinationDirectory: destination, cancellation: InstallCancellation()) { _ in }
        XCTAssertEqual(try String(contentsOf: installed.appendingPathComponent("Contents/MacOS/MSKey"), encoding: .utf8), "new version")
    }

    private func installer() -> AppInstaller { AppInstaller(validate: { _ in }) }

    private func fixture(at app: URL, text: String) throws {
        try FileManager.default.createDirectory(at: app.appendingPathComponent("Contents/MacOS"), withIntermediateDirectories: true)
        let plist = ["CFBundleIdentifier": "org.mskey.mac", "CFBundlePackageType": "APPL", "CFBundleExecutable": "MSKey"]
        try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0).write(to: app.appendingPathComponent("Contents/Info.plist"))
        let executable = app.appendingPathComponent("Contents/MacOS/MSKey")
        try Data(text.utf8).write(to: executable)
        XCTAssertEqual(chmod(executable.path, S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH), 0)
    }

    private func assertNoStagingFiles() throws {
        let names = try FileManager.default.contentsOfDirectory(atPath: destination.path)
        XCTAssertFalse(names.contains { $0.hasPrefix(".MSKey-install-") || $0.hasPrefix(".MSKey-backup-") })
    }
}

private final class EventRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var events: [InstallProgress] = []
    var values: [InstallProgress] { lock.withLock { events } }
    func record(_ event: InstallProgress) { lock.withLock { events.append(event) } }
}

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.withLock { count } }
    func next() -> Int { lock.withLock { count += 1; return count } }
}
