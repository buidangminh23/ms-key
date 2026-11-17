import AppKit
import Carbon
import SwiftUI

private final class SandboxNSTextView: NSTextView {
    let engine = RustEngine()
    var lastConfigSnapshot: MSKeyConfig?
    var onFocusChange: ((Bool) -> Void)?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.window?.makeFirstResponder(self)
        }
    }

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
    let clearRequest: Int
    let onFocusChange: (Bool) -> Void

    class Coordinator {
        var lastClearRequest: Int

        init(lastClearRequest: Int) {
            self.lastClearRequest = lastClearRequest
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(lastClearRequest: clearRequest)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.borderType = .noBorder

        let tv = SandboxNSTextView(frame: NSRect(x: 0, y: 0, width: 460, height: 120))
        tv.isEditable = true
        tv.isRichText = false
        tv.font = NSFont.systemFont(ofSize: 14)
        tv.textContainerInset = NSSize(width: 8, height: 8)
        tv.setAccessibilityLabel("Ô gõ thử")
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.isAutomaticDashSubstitutionEnabled = false
        tv.isAutomaticTextReplacementEnabled = false
        tv.isAutomaticSpellingCorrectionEnabled = false
        tv.autoresizingMask = [.width]
        tv.isVerticallyResizable = true
        tv.textContainer?.widthTracksTextView = true
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
            tv.engine.resetWord()
            tv.lastConfigSnapshot = config
        }
        if context.coordinator.lastClearRequest != clearRequest {
            context.coordinator.lastClearRequest = clearRequest
            tv.string = ""
            tv.engine.resetWord()
            tv.window?.makeFirstResponder(tv)
        }
    }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: Coordinator) {
        if let tv = scroll.documentView as? SandboxNSTextView {
            tv.onFocusChange?(false)
        }
    }
}

struct TypingSandboxView: View {
    @ObservedObject var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var method: InputMethodKind
    @State private var vietnamese: Bool
    @State private var clearRequest = 0

    init(store: MSKeyStore) {
        self.store = store
        _method = State(initialValue: store.config.input_method)
        _vietnamese = State(initialValue: store.config.vietnamese_enabled)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(
                title: "Gõ thử",
                detail: "Kiểu gõ, chế độ và văn bản thử chỉ dùng trong cửa sổ này."
            )
            HStack(spacing: 16) {
                Picker("Kiểu gõ", selection: $method) {
                    ForEach(InputMethodKind.allCases) { item in
                        Text(item.label).tag(item)
                    }
                }
                .fixedSize()
                Toggle("Bật Tiếng Việt", isOn: $vietnamese)
                Spacer(minLength: 8)
            }
            SettingsSection("Văn bản thử") {
                SandboxEditorRepresentable(config: sandboxConfig, clearRequest: clearRequest) { focused in
                    store.tapManager.setSandboxFocused(focused)
                }
                .frame(height: 196)
            }

            VStack(alignment: .leading, spacing: 4) {
                if !vietnamese {
                    Text("Bật Tiếng Việt để thử đặt dấu.")
                } else if let sample {
                    HStack(spacing: 8) {
                        Text("Gõ")
                        Text(sample).font(.body.monospaced())
                        Text("để ra Tiếng Việt.")
                    }
                } else {
                    Text("Gõ bằng các phím đã đặt trong kiểu gõ tự định nghĩa.")
                }
            }
            .foregroundStyle(.secondary)

            Divider()
            HStack(spacing: 8) {
                Button("Xóa văn bản thử") { clearRequest += 1 }
                Spacer(minLength: 8)
                Button("Đóng") { dismiss() }
            }
        }
        .settingsSheet(width: 600)
        .onDisappear { store.tapManager.setSandboxFocused(false) }
    }

    private var sandboxConfig: MSKeyConfig {
        var config = store.config
        config.input_method = method
        config.vietnamese_enabled = vietnamese
        return config
    }

    private var sample: String? {
        switch method {
        case .telex, .simpleTelex: return "Tieengs Vieejt"
        case .vni: return "Tie6ng1 Vie6t5"
        case .viqr: return "Tie^'ng Vie^.t"
        case .custom: return nil
        }
    }
}

struct AdvancedSettingsView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var statusMessage = ""

    init(store: MSKeyStore) {
        self.store = store
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(title: "Mở rộng & sao lưu")

            SettingsSection("Tùy chọn mở rộng") {
                Toggle("Gõ tắt Telex (cc→ch, gg→gi, nn→ng, pp→ph, qq→qu, tt→th)", isOn: binding(\.quick_telex))
                Toggle("Gõ tắt phụ âm đầu (f→ph, j→gi, w→qu)", isOn: binding(\.quick_start_consonant))
                Toggle("Gõ tắt phụ âm cuối (g→ng, h→nh, k→ch)", isOn: binding(\.quick_end_consonant))
                Toggle("Hiện chỉ báo nổi khi chuyển V/E", isOn: binding(\.show_switch_osd))
                Toggle("Dùng Shift + Insert khi dán clipboard", isOn: binding(\.paste_via_shift_insert))
                Divider()
                HStack(spacing: 8) {
                    Text("Độ trễ gửi phím")
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
                    .accessibilityLabel("Độ trễ gửi phím")
                    Spacer(minLength: 8)
                }
            }

            Divider()
            HStack(spacing: 8) {
                Button("Xuất JSON...") { exportConfig() }.frame(height: 28)
                Button("Nhập JSON...") { importConfig() }.frame(height: 28)
                if !statusMessage.isEmpty {
                    Text(statusMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 8)
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .settingsSheet(width: 600)
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
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(
                title: "Từ điển cá nhân",
                detail: "Giữ nguyên tên riêng và từ chuyên ngành khi kiểm tra chính tả."
            )

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
                        .frame(height: 24)
                    }
                }
            }
            .frame(height: 200)
            .overlay {
                if filteredWords.isEmpty {
                    Text(store.config.user_dict_words.isEmpty ? "Từ điển đang trống" : "Không tìm thấy từ")
                        .foregroundStyle(.secondary)
                }
            }

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
                .buttonStyle(.borderedProminent)
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

            Divider()
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
        .settingsSheet(width: 600)
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
