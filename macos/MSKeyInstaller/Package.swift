// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MSKeyInstaller",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "MSKeyInstaller", targets: ["MSKeyInstaller"])],
    targets: [
        .target(name: "InstallerCore"),
        .executableTarget(name: "MSKeyInstaller", dependencies: ["InstallerCore"]),
        .testTarget(name: "InstallerCoreTests", dependencies: ["InstallerCore"]),
    ]
)
