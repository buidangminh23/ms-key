import AppKit
import SwiftUI

@MainActor
public final class MenuBarController: NSObject {
    private var statusItem: NSStatusItem?
    private var menu = NSMenu()
    private weak var store: MSKeyStore?

    public init(store: MSKeyStore) {
        self.store = store
        super.init()
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.statusItem = item
        if let button = item.button {
            button.target = self
            button.action = #selector(onStatusClick)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }
        updateStatusIcon(vietnamese: store.effectiveVE)
        rebuildMenu()
    }

    public func updateStatusIcon(vietnamese: Bool) {
        guard let button = statusItem?.button else { return }
        button.image = Self.letterImage(letter: vietnamese ? "V" : "E", vietnamese: vietnamese)
        button.toolTip = vietnamese ? "MS Key — Tiếng Việt (V)" : "MS Key — Tiếng Anh (E)"
        rebuildMenu()
    }

    public func refreshProfilesMenu() {
        rebuildMenu()
    }

    public func rebuildMenu() {
        guard let store else { return }
        let menu = NSMenu()
        let toggleTitle = store.effectiveVE ? "Chuyển sang Tiếng Anh (E)" : "Chuyển sang Tiếng Việt (V)"
        menu.addItem(makeItem(toggleTitle, action: #selector(onToggleVE)))
        menu.addItem(makeItem("Mở Bảng điều khiển...", action: #selector(onOpenSettings)))
        menu.addItem(.separator())

        let methodMenu = NSMenu()
        for m in InputMethodKind.allCases {
            let it = makeItem(m.label, action: #selector(onPickMethod(_:)), rep: m.rawValue)
            it.state = store.config.input_method == m ? .on : .off
            methodMenu.addItem(it)
        }
        let methodRoot = NSMenuItem(title: "Kiểu gõ", action: nil, keyEquivalent: "")
        methodRoot.submenu = methodMenu
        menu.addItem(methodRoot)

        let charsetMenu = NSMenu()
        for cs in CharsetKind.allCases {
            let it = makeItem(cs.label, action: #selector(onPickCharset(_:)), rep: cs.rawValue)
            it.state = store.config.charset == cs ? .on : .off
            charsetMenu.addItem(it)
        }
        let charsetRoot = NSMenuItem(title: "Bảng mã", action: nil, keyEquivalent: "")
        charsetRoot.submenu = charsetMenu
        menu.addItem(charsetRoot)

        let profileMenu = NSMenu()
        for name in store.config.allProfileNames() {
            let it = makeItem(name, action: #selector(onPickProfile(_:)), rep: name)
            it.state = store.config.activeProfile.caseInsensitiveCompare(name) == .orderedSame ? .on : .off
            profileMenu.addItem(it)
        }
        let profileRoot = NSMenuItem(title: "Hồ sơ", action: nil, keyEquivalent: "")
        profileRoot.submenu = profileMenu
        menu.addItem(profileRoot)

        menu.addItem(.separator())
        menu.addItem(makeItem("Chuyển đổi vùng chọn", action: #selector(onConvertSelection)))
        menu.addItem(makeItem("Chuyển mã nhanh Clipboard", action: #selector(onQuickConvertClipboard)))
        menu.addItem(makeItem("Thêm từ Esc gần nhất", action: #selector(onAddLastEscWord)))
        menu.addItem(makeItem("Công cụ chuyển mã...", action: #selector(onOpenConverter)))
        menu.addItem(makeItem("Bảng gõ tắt...", action: #selector(onOpenMacros)))
        menu.addItem(makeItem("Bảng từ điển cá nhân...", action: #selector(onOpenUserDict)))
        menu.addItem(makeItem("Mở rộng & Gõ thử...", action: #selector(onOpenExtras)))
        let osdItem = makeItem("Hiện OSD chuyển V/E", action: #selector(onToggleOsd))
        osdItem.state = store.config.show_switch_osd ? .on : .off
        menu.addItem(osdItem)
        menu.addItem(.separator())
        menu.addItem(makeItem("Kiểm tra cập nhật...", action: #selector(onCheckUpdates)))
        menu.addItem(makeItem("Thoát MS Key", action: #selector(onQuit), key: "q"))
        self.menu = menu
    }

    @objc private func onStatusClick() {
        let event = NSApp.currentEvent
        let wantsMenu = event?.type == .rightMouseUp || event?.modifierFlags.contains(.control) == true
        guard wantsMenu, let item = statusItem, let button = item.button else {
            store?.toggleVE()
            return
        }
        item.menu = menu
        button.performClick(nil)
        item.menu = nil
    }

    private func makeItem(_ title: String, action: Selector, key: String = "", rep: Any? = nil) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        item.representedObject = rep
        return item
    }

    static let letterInkHeight: CGFloat = 16

    static func letterImage(letter: String, vietnamese: Bool) -> NSImage {
        let color = vietnamese
            ? NSColor(srgbRed: 0xD1 / 255, green: 0x34 / 255, blue: 0x38 / 255, alpha: 1)
            : NSColor(srgbRed: 0x00 / 255, green: 0x78 / 255, blue: 0xD4 / 255, alpha: 1)
        let probe = NSFont.systemFont(ofSize: 100, weight: .bold)
        let font = NSFont.systemFont(ofSize: 100 * letterInkHeight / probe.capHeight, weight: .bold)
        let text = NSAttributedString(string: letter, attributes: [.font: font, .foregroundColor: color])
        let ink = text.boundingRect(with: NSSize(width: 100, height: 100), options: [.usesDeviceMetrics])
        let size = NSSize(width: max(18, ceil(ink.width) + 2), height: 22)
        let image = NSImage(size: size, flipped: false) { rect in
            let baseline = NSPoint(x: rect.midX - ink.midX, y: rect.midY - ink.midY)
            text.draw(with: NSRect(origin: baseline, size: NSSize(width: 100, height: 100)), options: [])
            return true
        }
        image.isTemplate = false
        return image
    }

    @objc private func onToggleVE() { store?.toggleVE() }
    @objc private func onOpenSettings() { store?.showMainWindow() }
    @objc private func onOpenConverter() { store?.openSheet(.converter) }
    @objc private func onOpenMacros() { store?.openSheet(.macros) }
    @objc private func onOpenUserDict() { store?.openSheet(.userDict) }
    @objc private func onOpenExtras() { store?.openSheet(.extras) }
    @objc private func onToggleOsd() { store?.updateConfig { $0.show_switch_osd.toggle() } }
    @objc private func onQuickConvertClipboard() { store?.quickConvertPasteboard() }
    @objc private func onConvertSelection() { store?.triggerConvertSelection() }
    @objc private func onAddLastEscWord() { _ = store?.addLastEscWordToUserDict() }
    @objc private func onCheckUpdates() { store?.checkForUpdates(manual: true) }
    @objc private func onQuit() { NSApp.terminate(nil) }

    @objc private func onPickMethod(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let method = InputMethodKind(rawValue: raw) else { return }
        store?.updateConfig { $0.input_method = method }
    }

    @objc private func onPickCharset(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let cs = CharsetKind(rawValue: raw) else { return }
        store?.updateConfig { $0.charset = cs }
    }

    @objc private func onPickProfile(_ sender: NSMenuItem) {
        guard let name = sender.representedObject as? String else { return }
        _ = store?.applyProfile(name)
    }
}
