import AppKit
import SwiftUI

public struct MacroManagerView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var shortcut = ""
    @State private var expansion = ""
    @State private var statusText = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Bảng gõ tắt").font(.headline)
            List {
                ForEach(store.config.macros) { item in
                    HStack(spacing: 8) {
                        Text(item.shortcut).fontWeight(.semibold).frame(width: 110, alignment: .leading)
                        Text("→").foregroundStyle(.secondary)
                        Text(item.expansion.replacingOccurrences(of: "\n", with: "\\n")).lineLimit(1)
                        if !item.enabled {
                            Text("[Tắt]").font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button(item.enabled ? "Tắt" : "Bật") {
                            store.updateConfig { cfg in
                                if let idx = cfg.macros.firstIndex(where: { $0.shortcut == item.shortcut }) {
                                    cfg.macros[idx].enabled.toggle()
                                }
                            }
                        }
                        .frame(height: 24)
                        Button("Xoá") {
                            store.updateConfig { $0.macros.removeAll { $0.shortcut == item.shortcut } }
                        }
                        .frame(height: 24)
                    }
                    .opacity(item.enabled ? 1.0 : 0.55)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        shortcut = item.shortcut
                        expansion = item.expansion.replacingOccurrences(of: "\n", with: "\\n")
                    }
                }
            }
            .frame(height: 200)

            HStack(spacing: 8) {
                TextField("Từ tắt", text: $shortcut).frame(width: 110)
                TextField("Nội dung (dùng \\n để xuống dòng)", text: $expansion)
                Button("Thêm / Cập nhật") {
                    let s = shortcut.trimmingCharacters(in: .whitespaces)
                    let e = expansion.replacingOccurrences(of: "\\n", with: "\n")
                    guard !s.isEmpty else { return }
                    store.updateConfig { cfg in
                        if let idx = cfg.macros.firstIndex(where: { $0.shortcut == s }) {
                            let prevEnabled = cfg.macros[idx].enabled
                            cfg.macros[idx] = MacroItem(shortcut: s, expansion: e, enabled: prevEnabled)
                        } else {
                            cfg.macros.append(MacroItem(shortcut: s, expansion: e, enabled: true))
                        }
                    }
                    shortcut = ""
                    expansion = ""
                }
                .frame(height: 28)
            }

            HStack(spacing: 8) {
                Button("Nhập .txt...") { importMacrosTxt() }.frame(height: 28)
                Button("Xuất .txt...") { exportMacrosTxt() }.frame(height: 28)
                if !statusText.isEmpty {
                    Text(statusText).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 540)
    }

    private func importMacrosTxt() {
        let panel = NSOpenPanel()
        panel.title = "Nhập bảng gõ tắt (.txt)"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url,
              let raw = try? String(contentsOf: url, encoding: .utf8) else {
            return
        }
        let parsed = RustEngine.parseMacroText(raw)
        guard !parsed.isEmpty else {
            statusText = "Không tìm thấy mục gõ tắt hợp lệ"
            return
        }
        store.updateConfig { cfg in
            for item in parsed {
                if let idx = cfg.macros.firstIndex(where: { $0.shortcut == item.shortcut }) {
                    cfg.macros[idx] = item
                } else {
                    cfg.macros.append(item)
                }
            }
        }
        statusText = "Đã nhập \(parsed.count) mục"
    }

    private func exportMacrosTxt() {
        let panel = NSSavePanel()
        panel.title = "Xuất bảng gõ tắt (.txt)"
        panel.nameFieldStringValue = "mskey-macros.txt"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let body = RustEngine.formatMacroText(config: store.config)
        do {
            try body.write(to: url, atomically: true, encoding: .utf8)
            statusText = "Đã xuất \(url.lastPathComponent)"
        } catch {
            statusText = "Lỗi xuất file"
        }
    }
}

public struct AppRulesView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var processName = ""
    @State private var behavior: AppBehaviorKind = .forceEnglish

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Danh sách ứng dụng").font(.headline)
            Text("Hỗ trợ dấu * đại diện (vd: *code*, *.terminal) • Nhấn dòng để sửa")
                .font(.caption)
                .foregroundStyle(.secondary)
            List {
                ForEach(store.config.app_rules) { rule in
                    HStack(spacing: 8) {
                        Text(rule.process_name).frame(maxWidth: .infinity, alignment: .leading)
                        Text(rule.behavior.label).foregroundStyle(.secondary)
                        Button("Xoá") {
                            store.updateConfig { $0.app_rules.removeAll { $0.process_name == rule.process_name } }
                        }
                        .frame(height: 24)
                    }
                    .padding(.vertical, 2)
                    .background(
                        processName.caseInsensitiveCompare(rule.process_name) == .orderedSame
                            ? Color.accentColor.opacity(0.12)
                            : Color.clear,
                        in: RoundedRectangle(cornerRadius: 4)
                    )
                    .contentShape(Rectangle())
                    .onTapGesture {
                        processName = rule.process_name
                        behavior = rule.behavior
                    }
                }
            }
            .frame(height: 200)

            HStack(spacing: 8) {
                TextField("Tên app / binary (vd: Terminal)", text: $processName)
                Picker("", selection: $behavior) {
                    ForEach(AppBehaviorKind.allCases) { b in
                        Text(b.label).tag(b)
                    }
                }
                .labelsHidden()
                .frame(width: 160)
                Button("Lưu") {
                    let p = processName.trimmingCharacters(in: .whitespaces)
                    guard !p.isEmpty else { return }
                    store.updateConfig { cfg in
                        if let idx = cfg.app_rules.firstIndex(where: { $0.process_name.caseInsensitiveCompare(p) == .orderedSame }) {
                            cfg.app_rules[idx].behavior = behavior
                        } else {
                            cfg.app_rules.append(AppRuleItem(process_name: p, behavior: behavior))
                        }
                    }
                    processName = ""
                }
                .frame(height: 28)
            }

            HStack {
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 500)
    }
}

public struct CustomKeymapView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var map: CustomKeyMapModel = CustomKeyMapModel()

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Tự định nghĩa kiểu gõ").font(.headline)
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                row("Sắc / Huyền", $map.acute, $map.grave)
                row("Hỏi / Ngã", $map.hook, $map.tilde)
                row("Nặng / Xoá dấu", $map.dot, $map.remove_tone)
                row("Mũ â / ê", $map.circumflex_a, $map.circumflex_e)
                row("Mũ ô / Móc ơ", $map.circumflex_o, $map.horn_o)
                row("Móc ư / Trăng ă", $map.horn_u, $map.breve_a)
                row("Chữ đ / w -> ư", $map.d_stroke, $map.quick_u_horn)
                row("Móc nhanh [ ]", $map.quick_o_horn_key, $map.quick_u_horn_key)
            }
            HStack(spacing: 8) {
                Button("Mặc định Telex") { map = CustomKeyMapModel() }.frame(height: 32)
                Spacer()
                Button("Huỷ") { dismiss() }.frame(height: 32)
                Button("Áp dụng") {
                    store.updateConfig {
                        $0.custom_keymap = normalize(map)
                        $0.input_method = .custom
                    }
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 440)
        .onAppear { map = store.config.custom_keymap }
    }

    private func row(_ title: String, _ left: Binding<String>, _ right: Binding<String>) -> some View {
        GridRow {
            Text(title).frame(width: 140, alignment: .leading)
            TextField("", text: left).frame(width: 64)
            TextField("", text: right).frame(width: 64)
        }
    }

    private func normalize(_ m: CustomKeyMapModel) -> CustomKeyMapModel {
        var copy = m
        let single: (String, String) -> String = { v, d in
            v.first.map { String($0) } ?? d
        }
        copy.acute = single(m.acute, "s")
        copy.grave = single(m.grave, "f")
        copy.hook = single(m.hook, "r")
        copy.tilde = single(m.tilde, "x")
        copy.dot = single(m.dot, "j")
        copy.remove_tone = single(m.remove_tone, "z")
        copy.circumflex_a = single(m.circumflex_a, "a")
        copy.circumflex_e = single(m.circumflex_e, "e")
        copy.circumflex_o = single(m.circumflex_o, "o")
        copy.horn_o = single(m.horn_o, "w")
        copy.horn_u = single(m.horn_u, "w")
        copy.breve_a = single(m.breve_a, "w")
        copy.d_stroke = single(m.d_stroke, "d")
        copy.quick_u_horn = single(m.quick_u_horn, "w")
        copy.quick_o_horn_key = single(m.quick_o_horn_key, "[")
        copy.quick_u_horn_key = single(m.quick_u_horn_key, "]")
        return copy
    }
}

public struct ConverterView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var fromCharset: ConvertEncodingKind = .auto
    @State private var toCharset: ConvertEncodingKind = .unicode
    @State private var caseMode: UInt8 = 0
    @State private var text = ""
    @State private var detectedNote = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Chuyển mã Clipboard").font(.headline)
                Spacer()
                if !detectedNote.isEmpty {
                    Text(detectedNote)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            HStack(spacing: 8) {
                Picker("Nguồn", selection: $fromCharset) {
                    ForEach(ConvertEncodingKind.allCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Đích", selection: $toCharset) {
                    ForEach(ConvertEncodingKind.targetCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Chữ", selection: $caseMode) {
                    Text("Giữ nguyên").tag(UInt8(0))
                    Text("CHỮ HOA").tag(UInt8(1))
                    Text("chữ thường").tag(UInt8(2))
                    Text("Hoa đầu câu").tag(UInt8(3))
                    Text("Hoa Mỗi Từ").tag(UInt8(4))
                    Text("Bỏ dấu tiếng Việt").tag(UInt8(5))
                }
            }
            TextEditor(text: $text)
                .font(.body)
                .frame(height: 180)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.3)))

            HStack(spacing: 8) {
                Button("Lấy Clipboard") {
                    text = NSPasteboard.general.string(forType: .string) ?? ""
                    updateDetectedNote()
                }
                .frame(height: 32)

                Button("Chuyển mã") {
                    updateDetectedNote()
                    text = RustEngine.convertWithEncoding(text, from: fromCharset, to: toCharset, caseMode: caseMode)
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)

                Button("Chép Clipboard") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                }
                .frame(height: 32)

                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 560)
        .onAppear {
            text = NSPasteboard.general.string(forType: .string) ?? ""
            updateDetectedNote()
        }
    }

    private func updateDetectedNote() {
        guard !text.isEmpty else {
            detectedNote = ""
            return
        }
        if fromCharset == .auto {
            detectedNote = "Nhận diện: \(RustEngine.detectCharset(text).label)"
        } else {
            detectedNote = ""
        }
    }
}

public enum HotkeyCaptureTarget: Sendable {
    case switchHotkey
    case convertHotkey
}

public struct HotkeyCaptureView: View {
    @ObservedObject public var store: MSKeyStore
    public let target: HotkeyCaptureTarget
    @Environment(\.dismiss) private var dismiss
    @State private var captured: HotkeyComboModel = HotkeyComboModel()
    @State private var monitor: Any?

    public init(store: MSKeyStore, target: HotkeyCaptureTarget) {
        self.store = store
        self.target = target
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Nhấn tổ hợp phím mới...").font(.headline)
            Text("Hỗ trợ A-Z, 0-9, F1-F12, Space, Tab, Return, dấu câu")
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(captured.displayText)
                .font(.title3.monospaced())
                .padding(12)
                .frame(maxWidth: .infinity)
                .background(.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))

            HStack(spacing: 8) {
                Spacer()
                Button("Huỷ") { dismiss() }.frame(height: 32)
                Button("Lưu") {
                    store.updateConfig { cfg in
                        if target == .switchHotkey {
                            cfg.custom_switch_hotkey = captured
                            cfg.switch_preset = .custom
                        } else {
                            cfg.clipboard_convert_hotkey = captured
                        }
                    }
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 340)
        .onAppear {
            captured = target == .switchHotkey
                ? store.config.custom_switch_hotkey
                : store.config.clipboard_convert_hotkey
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { ev in
                let vk = MacKeycode.toPortableVK(ev.keyCode)
                let flags = ev.modifierFlags
                captured = HotkeyComboModel(
                    ctrl: flags.contains(.control),
                    alt: flags.contains(.option),
                    shift: flags.contains(.shift),
                    win_cmd: flags.contains(.command),
                    key_code: vk
                )
                return nil
            }
        }
        .onDisappear {
            if let m = monitor {
                NSEvent.removeMonitor(m)
                monitor = nil
            }
        }
    }
}
