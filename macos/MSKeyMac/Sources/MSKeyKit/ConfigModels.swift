import Foundation
import ServiceManagement

public enum InputMethodKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case telex = "Telex"
    case vni = "Vni"
    case viqr = "Viqr"
    case simpleTelex = "SimpleTelex"
    case custom = "Custom"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .telex: return "Telex"
        case .vni: return "VNI"
        case .viqr: return "VIQR"
        case .simpleTelex: return "Simple Telex"
        case .custom: return "Tự định nghĩa"
        }
    }
}

public enum CharsetKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case unicode = "Unicode"
    case tcvn3 = "Tcvn3"
    case vniWindows = "VniWindows"
    case unicodeComposite = "UnicodeComposite"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .unicode: return "Unicode"
        case .tcvn3: return "TCVN3 (ABC)"
        case .vniWindows: return "VNI Windows"
        case .unicodeComposite: return "Unicode tổ hợp"
        }
    }

    public var ffiCode: UInt8 {
        switch self {
        case .unicode: return 0
        case .tcvn3: return 1
        case .vniWindows: return 2
        case .unicodeComposite: return 3
        }
    }
}

public enum MechanismKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case keyboardHook = "KeyboardHook"
    case compatibleHook = "CompatibleHook"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .keyboardHook: return "CGEvent Hook (Chuẩn)"
        case .compatibleHook: return "Tương thích Clipboard/An toàn"
        }
    }
}

public enum ThemeModeKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case system = "System"
    case light = "Light"
    case dark = "Dark"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .system: return "Theo hệ thống"
        case .light: return "Sáng (Light)"
        case .dark: return "Tối (Dark)"
        }
    }
}

public enum SwitchPresetKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case ctrlShift = "CtrlShift"
    case altZ = "AltZ"
    case custom = "Custom"

    public var id: String { rawValue }
}

public enum AppBehaviorKind: String, Codable, CaseIterable, Sendable, Identifiable {
    case `default` = "Default"
    case forceEnglish = "ForceEnglish"
    case forceVietnamese = "ForceVietnamese"
    case blacklist = "Blacklist"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .default: return "Mặc định"
        case .forceEnglish: return "Luôn Tiếng Anh"
        case .forceVietnamese: return "Luôn Tiếng Việt"
        case .blacklist: return "Bỏ qua (Blacklist)"
        }
    }
}

public struct HotkeyComboModel: Codable, Equatable, Sendable {
    public var ctrl: Bool
    public var alt: Bool
    public var shift: Bool
    public var win_cmd: Bool
    public var key_code: UInt32

    public init(ctrl: Bool = true, alt: Bool = false, shift: Bool = true, win_cmd: Bool = false, key_code: UInt32 = 0x75) {
        self.ctrl = ctrl
        self.alt = alt
        self.shift = shift
        self.win_cmd = win_cmd
        self.key_code = key_code
    }

    public var displayText: String {
        var parts: [String] = []
        if ctrl { parts.append("Ctrl") }
        if alt { parts.append("Option") }
        if shift { parts.append("Shift") }
        if win_cmd { parts.append("Cmd") }
        let key: String
        switch key_code {
        case 0x70...0x7B:
            key = "F\(key_code - 0x6F)"
        case 0x30...0x5A:
            if let scalar = UnicodeScalar(key_code) {
                key = String(scalar)
            } else {
                key = String(format: "0x%X", key_code)
            }
        case 0x20:
            key = "Space"
        case 0xC0:
            key = "`"
        default:
            key = String(format: "0x%X", key_code)
        }
        parts.append(key)
        return parts.joined(separator: " + ")
    }
}

public struct AppRuleItem: Codable, Equatable, Sendable, Identifiable {
    public var process_name: String
    public var behavior: AppBehaviorKind

    public var id: String { process_name }

    public init(process_name: String, behavior: AppBehaviorKind) {
        self.process_name = process_name
        self.behavior = behavior
    }
}

public struct MacroItem: Codable, Equatable, Sendable, Identifiable {
    public var shortcut: String
    public var expansion: String
    public var enabled: Bool

    public var id: String { shortcut }

    public init(shortcut: String, expansion: String, enabled: Bool = true) {
        self.shortcut = shortcut
        self.expansion = expansion
        self.enabled = enabled
    }
}

public struct CustomKeyMapModel: Codable, Equatable, Sendable {
    public var acute: String = "s"
    public var grave: String = "f"
    public var hook: String = "r"
    public var tilde: String = "x"
    public var dot: String = "j"
    public var remove_tone: String = "z"
    public var circumflex_a: String = "a"
    public var circumflex_e: String = "e"
    public var circumflex_o: String = "o"
    public var horn_o: String = "w"
    public var horn_u: String = "w"
    public var breve_a: String = "w"
    public var d_stroke: String = "d"
    public var quick_u_horn: String = "w"
    public var quick_o_horn_key: String = "["
    public var quick_u_horn_key: String = "]"

    public init() {}
}

public struct MSKeyConfig: Codable, Equatable, Sendable {
    public var vietnamese_enabled: Bool = true
    public var charset: CharsetKind = .unicode
    public var input_method: InputMethodKind = .telex
    public var mechanism: MechanismKind = .keyboardHook
    public var modern_tone: Bool = true
    public var free_tone: Bool = true
    public var allow_fjwz: Bool = true
    public var auto_capitalize_sentence: Bool = false
    public var use_clipboard_send: Bool = false
    public var standard_key_send: Bool = false
    public var spelling_check: Bool = true
    public var auto_restore_wrong_word: Bool = true
    public var disable_on_non_us_layout: Bool = true
    public var fix_browser_suggest_1: Bool = true
    public var fix_browser_suggest_2: Bool = false
    public var macro_enabled: Bool = true
    public var macro_when_english: Bool = false
    public var quick_w_to_u_horn: Bool = true
    public var quick_brackets: Bool = true
    public var esc_restore: Bool = true
    public var switch_preset: SwitchPresetKind = .ctrlShift
    public var custom_switch_hotkey: HotkeyComboModel = HotkeyComboModel(ctrl: true, alt: true, shift: false, win_cmd: false, key_code: 0x56)
    public var clipboard_convert_hotkey: HotkeyComboModel = HotkeyComboModel()
    public var beep_on_switch: Bool = false
    public var auto_start: Bool = false
    public var run_as_admin: Bool = false
    public var show_on_startup: Bool = true
    public var theme: ThemeModeKind = .system
    public var smart_app_switch: Bool = true
    public var app_rules: [AppRuleItem] = []
    public var macros: [MacroItem] = []
    public var custom_keymap: CustomKeyMapModel = CustomKeyMapModel()

    public init() {}
}

public enum MacConfigStore {
    public static var configDirectory: URL {
        let base = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config", isDirectory: true)
        return base.appendingPathComponent("mskey", isDirectory: true)
    }

    public static var configFile: URL {
        configDirectory.appendingPathComponent("config.json")
    }

    public static func load() -> MSKeyConfig {
        guard let data = try? Data(contentsOf: configFile),
              let cfg = try? JSONDecoder().decode(MSKeyConfig.self, from: data) else {
            return MSKeyConfig()
        }
        return cfg
    }

    public static func save(_ config: MSKeyConfig) {
        try? FileManager.default.createDirectory(at: configDirectory, withIntermediateDirectories: true)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(config) {
            try? data.write(to: configFile, options: .atomic)
        }
    }

    public static func syncAutoStart(enable: Bool) {
        guard Bundle.main.bundleURL.pathExtension == "app" else { return }
        if #available(macOS 13.0, *) {
            do {
                if enable {
                    try SMAppService.mainApp.register()
                } else {
                    try SMAppService.mainApp.unregister()
                }
            } catch {
            }
        }
    }
}
