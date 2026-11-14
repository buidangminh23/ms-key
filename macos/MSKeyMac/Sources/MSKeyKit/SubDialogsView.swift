import AppKit
import SwiftUI

public struct MacroManagerView: View {
    @ObservedObject public var store: MSKeyStore
    @Environment(\.dismiss) private var dismiss
    @State private var shortcut = ""
    @State private var expansion = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Bảng gõ tắt").font(.headline)
            List {
                ForEach(store.config.macros) { item in
                    HStack(spacing: 8) {
                        Text(item.shortcut).fontWeight(.semibold).frame(width: 120, alignment: .leading)
                        Text("→").foregroundStyle(.secondary)
                        Text(item.expansion).lineLimit(1)
                        Spacer()
                        Button("Xoá") {
                            store.updateConfig { $0.macros.removeAll { $0.shortcut == item.shortcut } }
                        }
                        .frame(height: 24)
                    }
                }
            }
            .frame(height: 200)

            HStack(spacing: 8) {
                TextField("Từ tắt", text: $shortcut).frame(width: 120)
                TextField("Nội dung thay thế", text: $expansion)
                Button("Thêm / Cập nhật") {
                    let s = shortcut.trimmingCharacters(in: .whitespaces)
                    let e = expansion
                    guard !s.isEmpty else { return }
                    store.updateConfig { cfg in
                        if let idx = cfg.macros.firstIndex(where: { $0.shortcut == s }) {
                            cfg.macros[idx] = MacroItem(shortcut: s, expansion: e, enabled: true)
                        } else {
                            cfg.macros.append(MacroItem(shortcut: s, expansion: e, enabled: true))
                        }
                    }
                    shortcut = ""
                    expansion = ""
                }
                .frame(height: 28)
            }

            HStack {
                Spacer()
                Button("Đóng") { dismiss() }.frame(height: 32)
            }
        }
        .padding(16)
        .frame(width: 480)
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
    @State private var fromCharset: CharsetKind = .tcvn3
    @State private var toCharset: CharsetKind = .unicode
    @State private var caseMode: UInt8 = 0
    @State private var text = ""

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Chuyển mã Clipboard").font(.headline)
            HStack(spacing: 8) {
                Picker("Nguồn", selection: $fromCharset) {
                    ForEach(CharsetKind.allCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Đích", selection: $toCharset) {
                    ForEach(CharsetKind.allCases) { cs in Text(cs.label).tag(cs) }
                }
                Picker("Chữ", selection: $caseMode) {
                    Text("Giữ nguyên").tag(UInt8(0))
                    Text("CHỮ HOA").tag(UInt8(1))
                    Text("chữ thường").tag(UInt8(2))
                }
            }
            TextEditor(text: $text)
                .font(.body)
                .frame(height: 180)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.3)))

            HStack(spacing: 8) {
                Button("Lấy Clipboard") {
                    text = NSPasteboard.general.string(forType: .string) ?? ""
                }
                .frame(height: 32)

                Button("Chuyển mã") {
                    text = RustEngine.convertText(text, from: fromCharset, to: toCharset, caseMode: caseMode)
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
        .frame(width: 520)
        .onAppear {
            text = NSPasteboard.general.string(forType: .string) ?? ""
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
