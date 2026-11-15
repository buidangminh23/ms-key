import AppKit

@MainActor
enum MSKeyBrand {
    static var mark: NSImage {
        if let url = Bundle.main.url(forResource: "mskey-mark", withExtension: "png"),
           let image = NSImage(contentsOf: url) {
            return image
        }
        return NSApp.applicationIconImage
    }

    static var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "development"
    }
}
