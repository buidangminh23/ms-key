import AppKit
import SwiftUI

public struct MacroManagerView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var searchQuery = ""
    @State private var shortcut = ""
    @State private var expansion = ""
    @State private var statusText = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    private var filteredMacros: [MacroItem] {
        let q = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return store.config.macros }
        return store.config.macros.filter {
            $0.shortcut.localizedCaseInsensitiveContains(q)
                || $0.expansion.localizedCaseInsensitiveContains(q)
        }
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(title: "Bảng gõ tắt")
            TextField("Tìm kiếm từ tắt hoặc nội dung...", text: $searchQuery)
            List {
                ForEach(filteredMacros) { item in
                    HStack(spacing: 8) {
                        Button {
                            shortcut = item.shortcut
                            expansion = item.expansion.replacingOccurrences(of: "\n", with: "\\n")
                        } label: {
                            HStack(spacing: 8) {
                                Text(item.shortcut)
                                    .font(.body.monospaced().weight(.medium))
                                    .lineLimit(1)
                                    .frame(width: 104, alignment: .leading)
                                Text(item.expansion.replacingOccurrences(of: "\n", with: "\\n"))
                                    .lineLimit(1)
                                    .foregroundStyle(.secondary)
                                if !item.enabled {
                                    Text("Đã tắt").font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer(minLength: 0)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.borderless)
                        .accessibilityHint("Mở để sửa từ gõ tắt")
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
                }
            }
            .frame(height: 200)
            .overlay {
                if filteredMacros.isEmpty {
                    Text(store.config.macros.isEmpty ? "Chưa có từ gõ tắt" : "Không tìm thấy từ gõ tắt")
                        .foregroundStyle(.secondary)
                }
            }

            HStack(spacing: 8) {
                TextField("Từ tắt", text: $shortcut).frame(width: 104)
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
                .buttonStyle(.borderedProminent)
                .disabled(shortcut.trimmingCharacters(in: .whitespaces).isEmpty)
                .frame(height: 28)
            }

            Divider()
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
        .settingsSheet(width: 600)
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
    @State private var titlePattern = ""
    @State private var behavior: AppBehaviorKind = .forceEnglish
    @State private var runningWindows: [RunningWindowInfo] = []

    public init(store: MSKeyStore) {
        self.store = store
    }

    private func isSelectedRule(_ rule: AppRuleItem) -> Bool {
        rule.process_name.caseInsensitiveCompare(processName.trimmingCharacters(in: .whitespaces)) == .orderedSame
            && rule.titlePattern.caseInsensitiveCompare(titlePattern.trimmingCharacters(in: .whitespaces)) == .orderedSame
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(
                title: "Quy tắc theo ứng dụng",
                detail: "Dùng dấu * để khớp một phần tên ứng dụng hoặc tiêu đề cửa sổ, ví dụ *GitHub*."
            )
            List {
                ForEach(store.config.app_rules) { rule in
                    HStack(spacing: 8) {
                        Button {
                            processName = rule.process_name
                            titlePattern = rule.titlePattern
                            behavior = rule.behavior
                        } label: {
                            HStack(spacing: 8) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(rule.process_name).fontWeight(.medium)
                                    if !rule.titlePattern.isEmpty {
                                        Text(rule.titlePattern)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                Text(rule.behavior.label).foregroundStyle(.secondary)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.borderless)
                        .accessibilityHint("Mở để sửa quy tắc")
                        Button("Xoá") {
                            store.updateConfig { cfg in
                                cfg.app_rules.removeAll { $0.id == rule.id }
                            }
                        }
                        .frame(height: 24)
                    }
                    .padding(.vertical, 4)
                    .background(
                        isSelectedRule(rule) ? Color.accentColor.opacity(0.12) : Color.clear,
                        in: RoundedRectangle(cornerRadius: 4)
                    )
                }
            }
            .frame(height: 200)
            .overlay {
                if store.config.app_rules.isEmpty {
                    Text("Chưa có quy tắc ứng dụng").foregroundStyle(.secondary)
                }
            }

            HStack(spacing: 8) {
                Menu("Chọn cửa sổ đang mở...") {
                    if runningWindows.isEmpty {
                        Text("Không có cửa sổ nào")
                    } else {
                        ForEach(runningWindows) { win in
                            let label = win.windowTitle.isEmpty
                                ? win.processName
                                : "\(win.processName) — \(win.windowTitle)"
                            Button(label) {
                                processName = win.processName
                                titlePattern = win.windowTitle
                            }
                        }
                    }
                }
                Button("Làm mới") {
                    runningWindows = MacAppDetector.listRunningWindows()
                }
                .frame(height: 26)
                Spacer()
            }

            VStack(spacing: 8) {
                HStack(spacing: 8) {
                    TextField("Tên app / binary (vd: Code, Chrome)", text: $processName)
                    Picker("Chế độ gõ", selection: $behavior) {
                        ForEach(AppBehaviorKind.allCases) { b in
                            Text(b.label).tag(b)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 160)
                }
                HStack(spacing: 8) {
                    TextField("Mẫu tiêu đề cửa sổ (tùy chọn, vd: *GitHub*)", text: $titlePattern)
                    Button("Lưu") {
                        let p = processName.trimmingCharacters(in: .whitespaces)
                        let t = titlePattern.trimmingCharacters(in: .whitespaces)
                        guard !p.isEmpty else { return }
                        store.updateConfig { cfg in
                            if let idx = cfg.app_rules.firstIndex(where: {
                                $0.process_name.caseInsensitiveCompare(p) == .orderedSame
                                    && $0.titlePattern.caseInsensitiveCompare(t) == .orderedSame
                            }) {
                                cfg.app_rules[idx].behavior = behavior
                            } else {
                                cfg.app_rules.append(AppRuleItem(process_name: p, titlePattern: t, behavior: behavior))
                            }
                        }
                        processName = ""
                        titlePattern = ""
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(processName.trimmingCharacters(in: .whitespaces).isEmpty)
                    .frame(height: 28)
                }
            }

            HStack {
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .settingsSheet(width: 600)
        .onAppear {
            runningWindows = MacAppDetector.listRunningWindows()
        }
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
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(title: "Tự định nghĩa kiểu gõ", detail: "Nhập một phím cho mỗi dấu hoặc ký tự.")
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                row("Dấu sắc", $map.acute, "Dấu huyền", $map.grave)
                row("Dấu hỏi", $map.hook, "Dấu ngã", $map.tilde)
                row("Dấu nặng", $map.dot, "Xoá dấu", $map.remove_tone)
                row("Mũ â", $map.circumflex_a, "Mũ ê", $map.circumflex_e)
                row("Mũ ô", $map.circumflex_o, "Móc ơ", $map.horn_o)
                row("Móc ư", $map.horn_u, "Trăng ă", $map.breve_a)
                row("Chữ đ", $map.d_stroke, "Gõ nhanh ư", $map.quick_u_horn)
                row("Móc nhanh ơ", $map.quick_o_horn_key, "Móc nhanh ư", $map.quick_u_horn_key)
            }
            .settingsCard()
            Divider()
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
        .settingsSheet(width: 464)
        .onAppear { map = store.config.custom_keymap }
    }

    private func row(_ leftTitle: String, _ left: Binding<String>, _ rightTitle: String, _ right: Binding<String>) -> some View {
        GridRow {
            Text(leftTitle)
            TextField(leftTitle, text: left).labelsHidden().frame(width: 64)
            Text(rightTitle)
            TextField(rightTitle, text: right).labelsHidden().frame(width: 64)
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
    @State private var caseTransform: CaseTransformKind = .none
    @State private var text = ""
    @State private var detectedNote = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(title: "Chuyển mã")
            HStack(spacing: 12) {
                Picker("Mã nguồn", selection: $fromCharset) {
                    ForEach(ConvertEncodingKind.allCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Mã đích", selection: $toCharset) {
                    ForEach(ConvertEncodingKind.targetCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Kiểu chữ", selection: $caseTransform) {
                    ForEach(CaseTransformKind.allCases) { kind in
                        Text(kind.label).tag(kind)
                    }
                }
            }
            .settingsCard()
            SettingsSection("Văn bản") {
                TextEditor(text: $text)
                    .font(.body)
                    .frame(height: 184)
                    .accessibilityLabel("Văn bản cần chuyển mã")
                if !detectedNote.isEmpty {
                    Text(detectedNote).font(.caption).foregroundStyle(.secondary)
                }
            }

            Divider()
            HStack(spacing: 8) {
                Button("Dán từ Clipboard") {
                    text = NSPasteboard.general.string(forType: .string) ?? ""
                    updateDetectedNote()
                }
                .frame(height: 32)

                Button("Chuyển mã") {
                    updateDetectedNote()
                    text = RustEngine.convertWithEncoding(
                        text,
                        from: fromCharset,
                        to: toCharset,
                        caseTransform: caseTransform
                    )
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)

                Button("Chép kết quả") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                }
                .frame(height: 32)

                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .settingsSheet(width: 656)
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
    case undoHotkey
    case convertSelectionHotkey
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
        VStack(alignment: .leading, spacing: 16) {
            SettingsSheetTitle(
                title: "Đặt tổ hợp phím",
                detail: "Nhấn tổ hợp phím mới. Hỗ trợ A–Z, 0–9, F1–F12, Space, Tab, Return và dấu câu."
            )
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
                        switch target {
                        case .switchHotkey:
                            cfg.custom_switch_hotkey = captured
                            cfg.switch_preset = .custom
                        case .convertHotkey:
                            cfg.clipboard_convert_hotkey = captured
                        case .undoHotkey:
                            cfg.undoLastWordHotkey = captured
                        case .convertSelectionHotkey:
                            cfg.convertSelectionHotkey = captured
                        }
                    }
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .frame(height: 32)
            }
        }
        .settingsSheet(width: 384)
        .onAppear {
            switch target {
            case .switchHotkey:
                captured = store.config.custom_switch_hotkey
            case .convertHotkey:
                captured = store.config.clipboard_convert_hotkey
            case .undoHotkey:
                captured = store.config.undoLastWordHotkey
            case .convertSelectionHotkey:
                captured = store.config.convertSelectionHotkey
            }
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
