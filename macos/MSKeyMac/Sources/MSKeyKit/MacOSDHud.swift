import AppKit
import Foundation

public struct SelectionActionItem: Identifiable, Sendable {
    public let index: Int
    public let title: String
    public let caseTransform: CaseTransformKind

    public var id: Int { index }

    public static let allActions: [SelectionActionItem] = [
        SelectionActionItem(index: 1, title: "CHỮ HOA", caseTransform: .upper),
        SelectionActionItem(index: 2, title: "chữ thường", caseTransform: .lower),
        SelectionActionItem(index: 3, title: "Hoa đầu câu", caseTransform: .sentence),
        SelectionActionItem(index: 4, title: "Hoa Mỗi Từ", caseTransform: .title),
        SelectionActionItem(index: 5, title: "Bỏ dấu tiếng Việt", caseTransform: .stripDiacritics),
        SelectionActionItem(index: 6, title: "Chuẩn hóa dấu mới (hoà)", caseTransform: .normalizeModernTone),
        SelectionActionItem(index: 7, title: "Chuẩn hóa dấu cũ (hòa)", caseTransform: .normalizeClassicTone),
        SelectionActionItem(index: 8, title: "Tự động → Unicode", caseTransform: .none),
    ]
}

@MainActor
public final class MacOSDHud {
    public static let shared = MacOSDHud()

    private var panel: NSPanel?
    private var labelField: NSTextField?
    private var badgeView: NSView?
    private var caretPanel: NSPanel?
    private var caretLabelField: NSTextField?
    private var caretBadgeView: NSView?
    private var generation: UInt64 = 0

    private init() {}

    public static func axCaretBoundsInCocoa() -> CGRect? {
        MacAppDetector.axCaretBoundsInCocoa()
    }

    public static func anchorPointNearCaretOrCursor() -> NSPoint {
        MacAppDetector.anchorPointNearCaretOrCursor()
    }

    static func badgeColor(vietnamese: Bool) -> CGColor {
        (
            vietnamese
                ? NSColor(srgbRed: 0.80, green: 0.14, blue: 0.14, alpha: 0.94)
                : NSColor(srgbRed: 0.13, green: 0.42, blue: 0.80, alpha: 0.94)
        ).cgColor
    }

    static func screenContaining(_ point: NSPoint) -> NSScreen? {
        NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
    }

    static func clampedOrigin(forSize size: NSSize, preferred: NSPoint, anchor: NSPoint) -> NSPoint {
        guard let vis = screenContaining(anchor)?.visibleFrame else { return preferred }
        let x = min(max(preferred.x, vis.minX + 6), vis.maxX - size.width - 6)
        var y = preferred.y
        if y + size.height > vis.maxY - 6 {
            y = anchor.y - size.height - 20
        }
        y = min(max(y, vis.minY + 6), vis.maxY - size.height - 6)
        return NSPoint(x: x, y: y)
    }

    public func show(vietnamese: Bool, nearCaret: Bool = false) {
        generation &+= 1
        let currentGen = generation
        let delayMs = nearCaret ? 750 : 950

        if nearCaret {
            panel?.orderOut(nil)
            let p = ensureCaretPanel()
            caretBadgeView?.layer?.backgroundColor = Self.badgeColor(vietnamese: vietnamese)
            caretLabelField?.stringValue = vietnamese ? "V" : "E"
            let anchor = Self.anchorPointNearCaretOrCursor()
            let preferred = NSPoint(x: anchor.x + 8, y: anchor.y + 6)
            p.setFrameOrigin(Self.clampedOrigin(forSize: p.frame.size, preferred: preferred, anchor: anchor))
            p.alphaValue = 1.0
            p.orderFrontRegardless()
        } else {
            caretPanel?.orderOut(nil)
            let p = ensurePanel()
            badgeView?.layer?.backgroundColor = Self.badgeColor(vietnamese: vietnamese)
            labelField?.stringValue = vietnamese ? "V  |  Tiếng Việt" : "E  |  English"
            positionPanel(p)
            p.alphaValue = 1.0
            p.orderFrontRegardless()
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(delayMs)) { [weak self] in
            guard let self, self.generation == currentGen else { return }
            self.panel?.orderOut(nil)
            self.caretPanel?.orderOut(nil)
        }
    }

    private func makeFloatingHudPanel(size: NSSize, cornerRadius: CGFloat) -> (NSPanel, NSView) {
        let rect = NSRect(origin: .zero, size: size)
        let p = NSPanel(
            contentRect: rect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        p.level = .statusBar
        p.collectionBehavior = [.canJoinAllSpaces, .ignoresCycle, .stationary, .transient]
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.ignoresMouseEvents = true

        let container = NSView(frame: rect)
        container.wantsLayer = true
        container.layer?.cornerRadius = cornerRadius
        container.layer?.masksToBounds = true
        p.contentView = container
        return (p, container)
    }

    private func ensurePanel() -> NSPanel {
        if let panel {
            return panel
        }
        let (p, container) = makeFloatingHudPanel(size: NSSize(width: 208, height: 38), cornerRadius: 10)

        let logo = NSImageView(frame: NSRect(x: 8, y: 5, width: 28, height: 28))
        logo.image = MSKeyBrand.mark
        logo.imageScaling = .scaleProportionallyUpOrDown
        container.addSubview(logo)
        let tf = NSTextField(labelWithString: "")
        tf.frame = NSRect(x: 42, y: 8, width: 156, height: 22)
        tf.alignment = .center
        tf.font = NSFont.systemFont(ofSize: 13.5, weight: .semibold)
        tf.textColor = .white
        tf.backgroundColor = .clear
        container.addSubview(tf)

        self.panel = p
        self.badgeView = container
        self.labelField = tf
        return p
    }

    private func ensureCaretPanel() -> NSPanel {
        if let caretPanel {
            return caretPanel
        }
        let (p, container) = makeFloatingHudPanel(size: NSSize(width: 34, height: 22), cornerRadius: 7)
        let tf = NSTextField(labelWithString: "")
        tf.frame = NSRect(x: 0, y: 2, width: 34, height: 18)
        tf.alignment = .center
        tf.font = NSFont.systemFont(ofSize: 12.5, weight: .bold)
        tf.textColor = .white
        tf.backgroundColor = .clear
        container.addSubview(tf)

        self.caretPanel = p
        self.caretBadgeView = container
        self.caretLabelField = tf
        return p
    }

    private func positionPanel(_ p: NSPanel) {
        let mouse = NSEvent.mouseLocation
        guard let vis = Self.screenContaining(mouse)?.visibleFrame else { return }
        let x = vis.maxX - p.frame.width - 20
        let y = vis.maxY - p.frame.height - 16
        p.setFrameOrigin(NSPoint(x: x, y: y))
    }
}

@MainActor
public final class SelectionActionPanel: NSObject {
    public static let shared = SelectionActionPanel()

    private var panel: NSPanel?
    private var globalMouseMonitor: Any?
    private var localMouseMonitor: Any?
    private var onSelectAction: ((SelectionActionItem) -> Void)?
    private var onCancelAction: (() -> Void)?

    public var isVisible: Bool {
        panel?.isVisible == true
    }

    private override init() {
        super.init()
    }

    public func show(
        previewText: String,
        anchor: NSPoint,
        onSelect: @escaping (SelectionActionItem) -> Void,
        onCancel: @escaping () -> Void
    ) {
        dismiss(invokeCancel: false)
        self.onSelectAction = onSelect
        self.onCancelAction = onCancel

        let width: CGFloat = 276
        let rowHeight: CGFloat = 28
        let headerHeight: CGFloat = 38
        let footerHeight: CGFloat = 22
        let padding: CGFloat = 8
        let actions = SelectionActionItem.allActions
        let totalHeight = padding * 2 + headerHeight + CGFloat(actions.count) * rowHeight + footerHeight

        let rect = NSRect(x: 0, y: 0, width: width, height: totalHeight)
        let p = NSPanel(
            contentRect: rect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        p.level = .floating
        p.collectionBehavior = [.canJoinAllSpaces, .ignoresCycle, .transient]
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true

        let effect = NSVisualEffectView(frame: rect)
        effect.material = .popover
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.wantsLayer = true
        effect.layer?.cornerRadius = 11
        effect.layer?.masksToBounds = true
        effect.layer?.borderWidth = 0.5
        effect.layer?.borderColor = NSColor.separatorColor.cgColor

        let cleanedPreview = previewText
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let snippet = cleanedPreview.count > 28 ? String(cleanedPreview.prefix(28)) + "…" : cleanedPreview
        let headerLabel = NSTextField(labelWithString: "Chuyển đổi vùng chọn")
        headerLabel.frame = NSRect(x: 12, y: totalHeight - padding - 18, width: width - 24, height: 17)
        headerLabel.font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        headerLabel.textColor = .labelColor
        effect.addSubview(headerLabel)

        let previewLabel = NSTextField(labelWithString: snippet)
        previewLabel.frame = NSRect(x: 12, y: totalHeight - padding - 34, width: width - 24, height: 15)
        previewLabel.font = NSFont.systemFont(ofSize: 11)
        previewLabel.textColor = .secondaryLabelColor
        previewLabel.lineBreakMode = .byTruncatingTail
        effect.addSubview(previewLabel)

        for (offset, item) in actions.enumerated() {
            let y = totalHeight - padding - headerHeight - CGFloat(offset + 1) * rowHeight
            let btn = NSButton(frame: NSRect(x: 8, y: y + 1, width: width - 16, height: rowHeight - 2))
            btn.bezelStyle = .recessed
            btn.isBordered = false
            btn.alignment = .left
            btn.font = NSFont.systemFont(ofSize: 12.5)
            btn.title = "  [\(item.index)]  \(item.title)"
            btn.tag = item.index
            btn.target = self
            btn.action = #selector(actionButtonClicked(_:))
            btn.wantsLayer = true
            btn.layer?.cornerRadius = 6
            btn.contentTintColor = .labelColor
            effect.addSubview(btn)
        }

        let footerLabel = NSTextField(labelWithString: "Nhấn phím 1–8 để chọn • Esc để hủy")
        footerLabel.frame = NSRect(x: 12, y: 6, width: width - 24, height: 16)
        footerLabel.font = NSFont.systemFont(ofSize: 10.5)
        footerLabel.textColor = .tertiaryLabelColor
        effect.addSubview(footerLabel)

        p.contentView = effect
        let preferred = NSPoint(x: anchor.x + 8, y: anchor.y - totalHeight - 8)
        let origin = MacOSDHud.clampedOrigin(forSize: rect.size, preferred: preferred, anchor: anchor)
        p.setFrameOrigin(origin)
        p.orderFrontRegardless()
        self.panel = p

        installOutsideClickMonitors()
    }

    @objc private func actionButtonClicked(_ sender: NSButton) {
        triggerShortcutKey(sender.tag)
    }

    public func triggerShortcutKey(_ number: Int) {
        guard isVisible,
              let item = SelectionActionItem.allActions.first(where: { $0.index == number }) else {
            return
        }
        let callback = onSelectAction
        dismiss(invokeCancel: false)
        callback?(item)
    }

    public func cancel() {
        dismiss(invokeCancel: true)
    }

    private func dismiss(invokeCancel: Bool) {
        removeMonitors()
        panel?.orderOut(nil)
        panel = nil
        let cancelCb = onCancelAction
        onSelectAction = nil
        onCancelAction = nil
        if invokeCancel {
            cancelCb?()
        }
    }

    private func installOutsideClickMonitors() {
        removeMonitors()
        globalMouseMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, let p = self.panel, p.isVisible else { return }
                let loc = NSEvent.mouseLocation
                if !p.frame.contains(loc) {
                    self.dismiss(invokeCancel: true)
                }
            }
        }
        localMouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            if let self, let p = self.panel, p.isVisible, event.window !== p {
                self.dismiss(invokeCancel: true)
            }
            return event
        }
    }

    private func removeMonitors() {
        if let g = globalMouseMonitor {
            NSEvent.removeMonitor(g)
            globalMouseMonitor = nil
        }
        if let l = localMouseMonitor {
            NSEvent.removeMonitor(l)
            localMouseMonitor = nil
        }
    }
}
