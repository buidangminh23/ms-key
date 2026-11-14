// swift-tools-version: 6.0
import Foundation
import PackageDescription

let pkgDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let repoRoot = pkgDir.deletingLastPathComponent().deletingLastPathComponent()
let ffiLibDir = ProcessInfo.processInfo.environment["MSKEY_FFI_LIB_DIR"]
    ?? repoRoot.appendingPathComponent("target/release").path

let package = Package(
    name: "MSKeyMac",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "MSKeyKit", targets: ["MSKeyKit"]),
        .executable(name: "MSKeyMac", targets: ["MSKeyMac"]),
    ],
    targets: [
        .systemLibrary(
            name: "CMSKeyCore",
            path: "Sources/CMSKeyCore"
        ),
        .target(
            name: "MSKeyKit",
            dependencies: ["CMSKeyCore"],
            path: "Sources/MSKeyKit"
        ),
        .executableTarget(
            name: "MSKeyMac",
            dependencies: ["MSKeyKit"],
            path: "Sources/MSKeyMac",
            linkerSettings: [
                .unsafeFlags(["-L", ffiLibDir, "-lmskey_ffi"]),
            ]
        ),
        .testTarget(
            name: "MSKeyMacTests",
            dependencies: ["MSKeyKit"],
            path: "Tests/MSKeyMacTests",
            linkerSettings: [
                .unsafeFlags(["-L", ffiLibDir, "-lmskey_ffi"]),
            ]
        ),
    ]
)
