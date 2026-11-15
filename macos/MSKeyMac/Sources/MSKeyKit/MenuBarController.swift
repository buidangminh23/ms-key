import AppKit
import SwiftUI

@MainActor
public final class MenuBarController: NSObject {
    private var statusItem: NSStatusItem?
    private weak var store: MSKeyStore?

    public init(store: MSKeyStore) {
        self.store = store
        super.init()
        let item = NSStatusBar.system.statusItem(withLength: 40)
        self.statusItem = item
        updateStatusIcon(vietnamese: store.effectiveVE)
        rebuildMenu()
    }

    public func updateStatusIcon(vietnamese: Bool) {
        guard let button = statusItem?.button else { return }
        button.image = makeBadgeImage(letter: vietnamese ? "V" : "E", vietnamese: vietnamese)
        button.toolTip = vietnamese ? "MS Key — Tiếng Việt (V)" : "MS Key — Tiếng Anh (E)"
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

        menu.addItem(.separator())
        menu.addItem(makeItem("Chuyển mã nhanh Clipboard", action: #selector(onQuickConvertClipboard)))
        menu.addItem(makeItem("Công cụ chuyển mã...", action: #selector(onOpenConverter)))
        menu.addItem(makeItem("Bảng gõ tắt...", action: #selector(onOpenMacros)))
        menu.addItem(makeItem("Bảng từ điển cá nhân...", action: #selector(onOpenUserDict)))
        menu.addItem(makeItem("Mở rộng & Gõ thử...", action: #selector(onOpenExtras)))
        let osdItem = makeItem("Hiện OSD chuyển V/E", action: #selector(onToggleOsd))
        osdItem.state = store.config.show_switch_osd ? .on : .off
        menu.addItem(osdItem)
        menu.addItem(.separator())
        menu.addItem(makeItem("Thoát MS Key", action: #selector(onQuit), key: "q"))
        statusItem?.menu = menu
    }

    private func makeItem(_ title: String, action: Selector, key: String = "", rep: Any? = nil) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        item.representedObject = rep
        return item
    }

    private func makeBadgeImage(letter: String, vietnamese: Bool) -> NSImage {
        let size = NSSize(width: 36, height: 20)
        let img = NSImage(size: size)
        img.lockFocus()
        MSKeyBrand.mark.draw(in: NSRect(x: 0, y: 1, width: 20, height: 18))
        let rect = NSRect(x: 21, y: 2, width: 15, height: 16)
        let path = NSBezierPath(roundedRect: rect, xRadius: 4, yRadius: 4)
        let bg = vietnamese
            ? NSColor(srgbRed: 0.84, green: 0.17, blue: 0.15, alpha: 1.0)
            : NSColor(srgbRed: 0.14, green: 0.45, blue: 0.85, alpha: 1.0)
        bg.setFill()
        path.fill()
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11.5, weight: .bold),
            .foregroundColor: NSColor.white,
        ]
        let str = NSAttributedString(string: letter, attributes: attrs)
        let sz = str.size()
        let pt = NSPoint(x: rect.midX - sz.width / 2, y: rect.midY - sz.height / 2 - 0.5)
        str.draw(at: pt)
        img.unlockFocus()
        img.isTemplate = false
        return img
    }

    @objc private func onToggleVE() { store?.toggleVE() }
    @objc private func onOpenSettings() { store?.showMainWindow() }
    @objc private func onOpenConverter() { store?.openSheet(.converter) }
    @objc private func onOpenMacros() { store?.openSheet(.macros) }
    @objc private func onOpenUserDict() { store?.openSheet(.userDict) }
    @objc private func onOpenExtras() { store?.openSheet(.extras) }
    @objc private func onToggleOsd() { store?.updateConfig { $0.show_switch_osd.toggle() } }
    @objc private func onQuickConvertClipboard() { store?.quickConvertPasteboard() }
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
}
