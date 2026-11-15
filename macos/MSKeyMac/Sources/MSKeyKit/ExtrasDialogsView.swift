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
    @State private var searchQuery = ""
    @State private var newWord = ""
    @State private var statusText = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    private var filteredWords: [String] {
        let q = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return store.config.user_dict_words }
        return store.config.user_dict_words.filter { $0.localizedCaseInsensitiveContains(q) }
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Từ điển cá nhân").font(.headline)
            Text("Giữ nguyên tên riêng / từ chuyên ngành khi kiểm tra chính tả.")
                .font(.caption)
                .foregroundStyle(.secondary)

            TextField("Tìm kiếm trong từ điển...", text: $searchQuery)

            List {
                ForEach(filteredWords, id: \.self) { word in
                    HStack(spacing: 8) {
                        Text(word).frame(maxWidth: .infinity, alignment: .leading)
                        Button("Xoá") {
                            store.updateConfig { cfg in
                                cfg.user_dict_words.removeAll { $0.caseInsensitiveCompare(word) == .orderedSame }
                            }
                        }
                        .frame(height: 22)
                    }
                }
            }
            .frame(height: 190)

            HStack(spacing: 8) {
                TextField("Thêm từ mới...", text: $newWord)
                Button("Thêm") {
                    let w = newWord.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !w.isEmpty else { return }
                    store.updateConfig { cfg in
                        if !cfg.user_dict_words.contains(where: { $0.caseInsensitiveCompare(w) == .orderedSame }) {
                            cfg.user_dict_words.append(w)
                        }
                    }
                    newWord = ""
                    statusText = "Đã thêm từ mới"
                }
                .disabled(newWord.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .frame(height: 28)

                Button("Thêm từ Esc gần nhất") {
                    if let added = store.addLastEscWordToUserDict() {
                        statusText = "Đã thêm: \(added)"
                    } else {
                        statusText = "Chưa có từ Esc gần nhất"
                    }
                }
                .frame(height: 28)
            }

            HStack(spacing: 8) {
                Button("Nhập .txt...") { importDictTxt() }.frame(height: 28)
                Button("Xuất .txt...") { exportDictTxt() }.frame(height: 28)
                Button("Xoá hết") {
                    store.updateConfig { $0.user_dict_words.removeAll() }
                    statusText = "Đã xoá toàn bộ từ điển"
                }
                .frame(height: 28)
                if !statusText.isEmpty {
                    Text(statusText).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 520)
    }

    private func importDictTxt() {
        let panel = NSOpenPanel()
        panel.title = "Nhập từ điển cá nhân (.txt)"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url,
              let raw = try? String(contentsOf: url, encoding: .utf8) else {
            return
        }
        let parsed = MacConfigStore.parseUserDictText(raw)
        guard !parsed.isEmpty else {
            statusText = "Không tìm thấy từ hợp lệ"
            return
        }
        store.updateConfig { cfg in
            var seen = Set(cfg.user_dict_words.map { $0.lowercased() })
            for w in parsed {
                if seen.insert(w.lowercased()).inserted {
                    cfg.user_dict_words.append(w)
                }
            }
        }
        statusText = "Đã nhập \(parsed.count) từ"
    }

    private func exportDictTxt() {
        let panel = NSSavePanel()
        panel.title = "Xuất từ điển cá nhân (.txt)"
        panel.nameFieldStringValue = "mskey-user-dict.txt"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let body = MacConfigStore.formatUserDictText(store.config.user_dict_words)
        do {
            try body.write(to: url, atomically: true, encoding: .utf8)
            statusText = "Đã xuất \(url.lastPathComponent)"
        } catch {
            statusText = "Lỗi xuất file"
        }
    }
}
