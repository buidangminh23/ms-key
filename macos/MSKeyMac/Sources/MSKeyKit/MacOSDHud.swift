import AppKit
import Foundation

@MainActor
public final class MacOSDHud {
    public static let shared = MacOSDHud()

    private var panel: NSPanel?
    private var labelField: NSTextField?
    private var badgeView: NSView?
    private var generation: UInt64 = 0

    private init() {}

    public func show(vietnamese: Bool) {
        generation &+= 1
        let currentGen = generation
        let p = ensurePanel()
        if let badge = badgeView {
            badge.layer?.backgroundColor = (
                vietnamese
                    ? NSColor(srgbRed: 0.80, green: 0.14, blue: 0.14, alpha: 0.94)
                    : NSColor(srgbRed: 0.13, green: 0.42, blue: 0.80, alpha: 0.94)
            ).cgColor
        }
        labelField?.stringValue = vietnamese ? "V  |  Tiếng Việt" : "E  |  English"
        positionPanel(p)
        p.alphaValue = 1.0
        p.orderFrontRegardless()

        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(950)) { [weak self] in
            guard let self, self.generation == currentGen else { return }
            self.panel?.orderOut(nil)
        }
    }

    private func ensurePanel() -> NSPanel {
        if let panel {
            return panel
        }
        let rect = NSRect(x: 0, y: 0, width: 208, height: 38)
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
        container.layer?.cornerRadius = 10
        container.layer?.masksToBounds = true

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

        p.contentView = container
        self.panel = p
        self.badgeView = container
        self.labelField = tf
        return p
    }

    private func positionPanel(_ p: NSPanel) {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        guard let vis = screen?.visibleFrame else { return }
        let x = vis.maxX - p.frame.width - 20
        let y = vis.maxY - p.frame.height - 16
        p.setFrameOrigin(NSPoint(x: x, y: y))
    }
}
