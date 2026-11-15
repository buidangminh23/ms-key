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

public enum ConvertEncodingKind: UInt8, CaseIterable, Sendable, Identifiable {
    case auto = 255
    case unicode = 0
    case tcvn3 = 1
    case vniWindows = 2
    case unicodeComposite = 3
    case viqr = 4

    public var id: UInt8 { rawValue }

    public var label: String {
        switch self {
        case .auto: return "Tự động (Auto)"
        case .unicode: return "Unicode"
        case .tcvn3: return "TCVN3 (ABC)"
        case .vniWindows: return "VNI Windows"
        case .unicodeComposite: return "Unicode tổ hợp"
        case .viqr: return "VIQR"
        }
    }

    public static var targetCases: [ConvertEncodingKind] {
        [.unicode, .tcvn3, .vniWindows, .unicodeComposite, .viqr]
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
    case ctrlSpace = "CtrlSpace"
    case shiftShift = "ShiftShift"
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
        parts.append(Self.keyName(for: key_code))
        return parts.joined(separator: " + ")
    }

    private static func keyName(for vk: UInt32) -> String {
        switch vk {
        case 0x70...0x7B: return "F\(vk - 0x6F)"
        case 0x30...0x5A:
            if let scalar = UnicodeScalar(vk) { return String(scalar) }
            return String(format: "0x%X", vk)
        case 0x20: return "Space"
        case 0x0D: return "Return"
        case 0x09: return "Tab"
        case 0xBA: return ";"
        case 0xBB: return "="
        case 0xBC: return ","
        case 0xBD: return "-"
        case 0xBE: return "."
        case 0xBF: return "/"
        case 0xC0: return "`"
        case 0xDB: return "["
        case 0xDC: return "\\"
        case 0xDD: return "]"
        case 0xDE: return "'"
        default: return String(format: "0x%X", vk)
        }
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

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = CustomKeyMapModel()
        acute = try c.decodeIfPresent(String.self, forKey: .acute) ?? d.acute
        grave = try c.decodeIfPresent(String.self, forKey: .grave) ?? d.grave
        hook = try c.decodeIfPresent(String.self, forKey: .hook) ?? d.hook
        tilde = try c.decodeIfPresent(String.self, forKey: .tilde) ?? d.tilde
        dot = try c.decodeIfPresent(String.self, forKey: .dot) ?? d.dot
        remove_tone = try c.decodeIfPresent(String.self, forKey: .remove_tone) ?? d.remove_tone
        circumflex_a = try c.decodeIfPresent(String.self, forKey: .circumflex_a) ?? d.circumflex_a
        circumflex_e = try c.decodeIfPresent(String.self, forKey: .circumflex_e) ?? d.circumflex_e
        circumflex_o = try c.decodeIfPresent(String.self, forKey: .circumflex_o) ?? d.circumflex_o
        horn_o = try c.decodeIfPresent(String.self, forKey: .horn_o) ?? d.horn_o
        horn_u = try c.decodeIfPresent(String.self, forKey: .horn_u) ?? d.horn_u
        breve_a = try c.decodeIfPresent(String.self, forKey: .breve_a) ?? d.breve_a
        d_stroke = try c.decodeIfPresent(String.self, forKey: .d_stroke) ?? d.d_stroke
        quick_u_horn = try c.decodeIfPresent(String.self, forKey: .quick_u_horn) ?? d.quick_u_horn
        quick_o_horn_key = try c.decodeIfPresent(String.self, forKey: .quick_o_horn_key) ?? d.quick_o_horn_key
        quick_u_horn_key = try c.decodeIfPresent(String.self, forKey: .quick_u_horn_key) ?? d.quick_u_horn_key
    }
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
    public var quick_telex: Bool = false
    public var quick_start_consonant: Bool = false
    public var quick_end_consonant: Bool = false
    public var esc_restore: Bool = true
    public var switch_preset: SwitchPresetKind = .ctrlShift
    public var custom_switch_hotkey: HotkeyComboModel = HotkeyComboModel(ctrl: true, alt: true, shift: false, win_cmd: false, key_code: 0x56)
    public var clipboard_convert_hotkey: HotkeyComboModel = HotkeyComboModel()
    public var beep_on_switch: Bool = false
    public var show_switch_osd: Bool = true
    public var paste_via_shift_insert: Bool = false
    public var send_delay_ms: UInt8 = 2
    public var auto_start: Bool = true
    public var run_as_admin: Bool = false
    public var show_on_startup: Bool = false
    public var theme: ThemeModeKind = .system
    public var smart_app_switch: Bool = true
    public var app_rules: [AppRuleItem] = []
    public var macros: [MacroItem] = []
    public var user_dict_words: [String] = []
    public var custom_keymap: CustomKeyMapModel = CustomKeyMapModel()

    public init() {}

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = MSKeyConfig()
        vietnamese_enabled = try c.decodeIfPresent(Bool.self, forKey: .vietnamese_enabled) ?? d.vietnamese_enabled
        charset = try c.decodeIfPresent(CharsetKind.self, forKey: .charset) ?? d.charset
        input_method = try c.decodeIfPresent(InputMethodKind.self, forKey: .input_method) ?? d.input_method
        mechanism = try c.decodeIfPresent(MechanismKind.self, forKey: .mechanism) ?? d.mechanism
        modern_tone = try c.decodeIfPresent(Bool.self, forKey: .modern_tone) ?? d.modern_tone
        free_tone = try c.decodeIfPresent(Bool.self, forKey: .free_tone) ?? d.free_tone
        allow_fjwz = try c.decodeIfPresent(Bool.self, forKey: .allow_fjwz) ?? d.allow_fjwz
        auto_capitalize_sentence = try c.decodeIfPresent(Bool.self, forKey: .auto_capitalize_sentence) ?? d.auto_capitalize_sentence
        use_clipboard_send = try c.decodeIfPresent(Bool.self, forKey: .use_clipboard_send) ?? d.use_clipboard_send
        standard_key_send = try c.decodeIfPresent(Bool.self, forKey: .standard_key_send) ?? d.standard_key_send
        spelling_check = try c.decodeIfPresent(Bool.self, forKey: .spelling_check) ?? d.spelling_check
        auto_restore_wrong_word = try c.decodeIfPresent(Bool.self, forKey: .auto_restore_wrong_word) ?? d.auto_restore_wrong_word
        disable_on_non_us_layout = try c.decodeIfPresent(Bool.self, forKey: .disable_on_non_us_layout) ?? d.disable_on_non_us_layout
        fix_browser_suggest_1 = try c.decodeIfPresent(Bool.self, forKey: .fix_browser_suggest_1) ?? d.fix_browser_suggest_1
        fix_browser_suggest_2 = try c.decodeIfPresent(Bool.self, forKey: .fix_browser_suggest_2) ?? d.fix_browser_suggest_2
        macro_enabled = try c.decodeIfPresent(Bool.self, forKey: .macro_enabled) ?? d.macro_enabled
        macro_when_english = try c.decodeIfPresent(Bool.self, forKey: .macro_when_english) ?? d.macro_when_english
        quick_w_to_u_horn = try c.decodeIfPresent(Bool.self, forKey: .quick_w_to_u_horn) ?? d.quick_w_to_u_horn
        quick_brackets = try c.decodeIfPresent(Bool.self, forKey: .quick_brackets) ?? d.quick_brackets
        quick_telex = try c.decodeIfPresent(Bool.self, forKey: .quick_telex) ?? d.quick_telex
        quick_start_consonant = try c.decodeIfPresent(Bool.self, forKey: .quick_start_consonant) ?? d.quick_start_consonant
        quick_end_consonant = try c.decodeIfPresent(Bool.self, forKey: .quick_end_consonant) ?? d.quick_end_consonant
        esc_restore = try c.decodeIfPresent(Bool.self, forKey: .esc_restore) ?? d.esc_restore
        switch_preset = try c.decodeIfPresent(SwitchPresetKind.self, forKey: .switch_preset) ?? d.switch_preset
        custom_switch_hotkey = try c.decodeIfPresent(HotkeyComboModel.self, forKey: .custom_switch_hotkey) ?? d.custom_switch_hotkey
        clipboard_convert_hotkey = try c.decodeIfPresent(HotkeyComboModel.self, forKey: .clipboard_convert_hotkey) ?? d.clipboard_convert_hotkey
        beep_on_switch = try c.decodeIfPresent(Bool.self, forKey: .beep_on_switch) ?? d.beep_on_switch
        show_switch_osd = try c.decodeIfPresent(Bool.self, forKey: .show_switch_osd) ?? d.show_switch_osd
        paste_via_shift_insert = try c.decodeIfPresent(Bool.self, forKey: .paste_via_shift_insert) ?? d.paste_via_shift_insert
        send_delay_ms = try c.decodeIfPresent(UInt8.self, forKey: .send_delay_ms) ?? d.send_delay_ms
        auto_start = try c.decodeIfPresent(Bool.self, forKey: .auto_start) ?? d.auto_start
        run_as_admin = try c.decodeIfPresent(Bool.self, forKey: .run_as_admin) ?? d.run_as_admin
        show_on_startup = try c.decodeIfPresent(Bool.self, forKey: .show_on_startup) ?? d.show_on_startup
        theme = try c.decodeIfPresent(ThemeModeKind.self, forKey: .theme) ?? d.theme
        smart_app_switch = try c.decodeIfPresent(Bool.self, forKey: .smart_app_switch) ?? d.smart_app_switch
        app_rules = try c.decodeIfPresent([AppRuleItem].self, forKey: .app_rules) ?? d.app_rules
        macros = try c.decodeIfPresent([MacroItem].self, forKey: .macros) ?? d.macros
        user_dict_words = try c.decodeIfPresent([String].self, forKey: .user_dict_words) ?? d.user_dict_words
        custom_keymap = try c.decodeIfPresent(CustomKeyMapModel.self, forKey: .custom_keymap) ?? d.custom_keymap
    }
}

public enum MacConfigStore {
    public static var configDirectory: URL {
        let base = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config", isDirectory: true)
        return base.appendingPathComponent("mskey", isDirectory: true)
    }

    public static var configFile: URL {
        configDirectory.appendingPathComponent("config.json")
    }

    public static var appStateFile: URL {
        configDirectory.appendingPathComponent("app_state.json")
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

    public static func loadAppState() -> [String: Bool] {
        guard let data = try? Data(contentsOf: appStateFile),
              let map = try? JSONDecoder().decode([String: Bool].self, from: data) else {
            return [:]
        }
        return map
    }

    public static func saveAppState(_ state: [String: Bool]) {
        try? FileManager.default.createDirectory(at: configDirectory, withIntermediateDirectories: true)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(state) {
            try? data.write(to: appStateFile, options: .atomic)
        }
    }

    public static func exportJSON(to url: URL, config: MSKeyConfig) -> Bool {
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? enc.encode(config) else { return false }
        return (try? data.write(to: url, options: .atomic)) != nil
    }

    public static func importJSON(from url: URL) -> MSKeyConfig? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(MSKeyConfig.self, from: data)
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
