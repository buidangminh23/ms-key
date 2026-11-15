import AppKit
import Carbon
import SwiftUI

private final class SandboxNSTextView: NSTextView {
    let engine = RustEngine()
    var lastConfigSnapshot: MSKeyConfig?
    var onFocusChange: ((Bool) -> Void)?

    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok { onFocusChange?(true) }
        return ok
    }

    override func resignFirstResponder() -> Bool {
        engine.resetWord()
        onFocusChange?(false)
        return super.resignFirstResponder()
    }

    override func mouseDown(with event: NSEvent) {
        engine.resetWord()
        super.mouseDown(with: event)
    }

    override func keyDown(with event: NSEvent) {
        let mods = event.modifierFlags.intersection([.command, .control, .option])
        if !mods.isEmpty {
            engine.resetWord()
            super.keyDown(with: event)
            return
        }
        let kc = event.keyCode
        if Int(kc) == kVK_Escape {
            apply(engine.processEscape()) { super.keyDown(with: event) }
            return
        }
        if Int(kc) == kVK_Delete {
            apply(engine.processBackspace()) { super.keyDown(with: event) }
            return
        }
        if MacKeycode.isNavigationKey(kc) {
            engine.resetWord()
            super.keyDown(with: event)
            return
        }
        let scalar: UInt32?
        if Int(kc) == kVK_Return || Int(kc) == kVK_ANSI_KeypadEnter {
            scalar = 10
        } else if Int(kc) == kVK_Tab {
            scalar = 9
        } else {
            scalar = event.characters?.unicodeScalars.first?.value
        }
        guard let sc = scalar else {
            engine.resetWord()
            super.keyDown(with: event)
            return
        }
        apply(engine.processKey(scalar: sc)) { super.keyDown(with: event) }
    }

    private func apply(_ act: KeyActionResult, fallback: () -> Void) {
        guard act.replace else {
            fallback()
            return
        }
        let full = self.string
        let sel = self.selectedRange()
        guard let selRange = Range(sel, in: full) else {
            fallback()
            return
        }
        let scalars = full[..<selRange.lowerBound].unicodeScalars
        var startIdx = scalars.endIndex
        var remaining = act.backspaces
        while remaining > 0 && startIdx > scalars.startIndex {
            startIdx = scalars.index(before: startIdx)
            remaining -= 1
        }
        let replaceNSRange = NSRange(startIdx..<selRange.upperBound, in: full)
        if shouldChangeText(in: replaceNSRange, replacementString: act.text) {
            replaceCharacters(in: replaceNSRange, with: act.text)
            didChangeText()
        }
    }
}

private struct SandboxEditorRepresentable: NSViewRepresentable {
    let config: MSKeyConfig
    let onFocusChange: (Bool) -> Void

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder

        let tv = SandboxNSTextView(frame: NSRect(x: 0, y: 0, width: 460, height: 120))
        tv.isEditable = true
        tv.isRichText = false
        tv.font = NSFont.systemFont(ofSize: 14)
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.isAutomaticDashSubstitutionEnabled = false
        tv.isAutomaticTextReplacementEnabled = false
        tv.isAutomaticSpellingCorrectionEnabled = false
        tv.autoresizingMask = [.width]
        tv.onFocusChange = onFocusChange
        tv.engine.setConfig(config)
        tv.lastConfigSnapshot = config

        scroll.documentView = tv
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let tv = scroll.documentView as? SandboxNSTextView else { return }
        tv.onFocusChange = onFocusChange
        if tv.lastConfigSnapshot != config {
            tv.engine.setConfig(config)
            tv.lastConfigSnapshot = config
        }
    }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: ()) {
        if let tv = scroll.documentView as? SandboxNSTextView {
            tv.onFocusChange?(false)
        }
    }
}

public struct ExtrasSandboxView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var statusMessage = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Mở rộng, Sao lưu & Gõ thử").font(.headline)

            GroupBox("Tùy chọn mở rộng") {
                VStack(alignment: .leading, spacing: 8) {
                    Toggle("Gõ tắt Telex (cc→ch, gg→gi, nn→ng, pp→ph, qq→qu, tt→th)", isOn: binding(\.quick_telex))
                    Toggle("Gõ tắt phụ âm đầu (f→ph, j→gi, w→qu)", isOn: binding(\.quick_start_consonant))
                    Toggle("Gõ tắt phụ âm cuối (g→ng, h→nh, k→ch)", isOn: binding(\.quick_end_consonant))
                    Toggle("Hiện OSD nổi khi chuyển V/E", isOn: binding(\.show_switch_osd))
                    Toggle("Dùng Shift+Insert khi dán clipboard", isOn: binding(\.paste_via_shift_insert))
                    HStack(spacing: 8) {
                        Text("Độ trễ gửi phím (ms):")
                        Stepper(
                            value: Binding(
                                get: { Int(store.config.send_delay_ms) },
                                set: { v in store.updateConfig { $0.send_delay_ms = UInt8(clamping: min(max(v, 0), 25)) } }
                            ),
                            in: 0...25
                        ) {
                            Text("\(store.config.send_delay_ms) ms")
                                .monospacedDigit()
                                .frame(width: 52, alignment: .trailing)
                        }
                        Spacer()
                    }
                }
                .padding(8)
            }

            HStack(spacing: 8) {
                Button("Xuất JSON...") { exportConfig() }.frame(height: 28)
                Button("Nhập JSON...") { importConfig() }.frame(height: 28)
                if !statusMessage.isEmpty {
                    Text(statusMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer()
            }

            GroupBox("Vùng gõ thử trực tiếp (tự xử lý Tiếng Việt ngay trong cửa sổ)") {
                SandboxEditorRepresentable(config: store.config) { focused in
                    store.tapManager.setSandboxFocused(focused)
                }
                .frame(height: 124)
                .padding(4)
            }

            HStack {
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 540)
        .onDisappear {
            store.tapManager.setSandboxFocused(false)
        }
    }

    private func binding<T>(_ keyPath: WritableKeyPath<MSKeyConfig, T>) -> Binding<T> {
        Binding(
            get: { store.config[keyPath: keyPath] },
            set: { newVal in
                store.updateConfig { $0[keyPath: keyPath] = newVal }
            }
        )
    }

    private func exportConfig() {
        let panel = NSSavePanel()
        panel.title = "Xuất cấu hình MS Key (JSON)"
        panel.nameFieldStringValue = "mskey-config.json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if MacConfigStore.exportJSON(to: url, config: store.config) {
            statusMessage = "Đã xuất \(url.lastPathComponent)"
        } else {
            statusMessage = "Lỗi xuất file JSON"
        }
    }

    private func importConfig() {
        let panel = NSOpenPanel()
        panel.title = "Nhập cấu hình MS Key (JSON)"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let loaded = MacConfigStore.importJSON(from: url) {
            store.updateConfig { $0 = loaded }
            statusMessage = "Đã nhập \(url.lastPathComponent)"
        } else {
            statusMessage = "File JSON không hợp lệ"
        }
    }
}

public struct UserDictView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Từ điển cá nhân").font(.headline)
            Text("Mỗi dòng một từ (giúp giữ nguyên tên riêng/từ chuyên ngành khi kiểm tra chính tả):")
                .font(.caption)
                .foregroundStyle(.secondary)
            TextEditor(text: $text)
                .font(.body)
                .frame(height: 220)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.3)))

            HStack(spacing: 8) {
                Button("Xoá hết") { text = "" }.frame(height: 32)
                Spacer()
                Button("Huỷ") { dismiss() }.frame(height: 32)
                Button("Lưu") {
                    var seen = Set<String>()
                    var cleaned: [String] = []
                    for line in text.components(separatedBy: .newlines) {
                        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
                        let key = trimmed.lowercased()
                        if !trimmed.isEmpty && seen.insert(key).inserted {
                            cleaned.append(trimmed)
                        }
                    }
                    store.updateConfig { $0.user_dict_words = cleaned }
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 440)
        .onAppear {
            text = store.config.user_dict_words.joined(separator: "\n")
        }
    }
}
