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

public enum CaseTransformKind: UInt8, CaseIterable, Sendable, Identifiable {
    case none = 0
    case upper = 1
    case lower = 2
    case sentence = 3
    case title = 4
    case stripDiacritics = 5
    case normalizeModernTone = 6
    case normalizeClassicTone = 7

    public var id: UInt8 { rawValue }

    public var label: String {
        switch self {
        case .none: return "Giữ nguyên"
        case .upper: return "CHỮ HOA"
        case .lower: return "chữ thường"
        case .sentence: return "Hoa đầu câu"
        case .title: return "Hoa Mỗi Từ"
        case .stripDiacritics: return "Bỏ dấu tiếng Việt"
        case .normalizeModernTone: return "Chuẩn hóa dấu mới (hoà)"
        case .normalizeClassicTone: return "Chuẩn hóa dấu cũ (hòa)"
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

public enum HoldEnglishKey: String, Codable, CaseIterable, Sendable, Identifiable {
    case none = "None"
    case rightShift = "RightShift"
    case leftShift = "LeftShift"
    case rightAlt = "RightAlt"
    case rightCtrl = "RightCtrl"
    case capsLock = "CapsLock"

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .none: return "Không dùng"
        case .rightShift: return "Shift phải"
        case .leftShift: return "Shift trái"
        case .rightAlt: return "Option / Alt phải"
        case .rightCtrl: return "Ctrl phải"
        case .capsLock: return "Caps Lock"
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
    public var titlePattern: String
    public var behavior: AppBehaviorKind

    public var id: String {
        "\(process_name.lowercased())\u{001F}\(titlePattern.lowercased())"
    }

    enum CodingKeys: String, CodingKey {
        case process_name
        case titlePattern = "title_pattern"
        case behavior
    }

    public init(process_name: String, titlePattern: String = "", behavior: AppBehaviorKind) {
        self.process_name = process_name
        self.titlePattern = titlePattern
        self.behavior = behavior
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        process_name = try c.decodeIfPresent(String.self, forKey: .process_name) ?? ""
        titlePattern = try c.decodeIfPresent(String.self, forKey: .titlePattern) ?? ""
        behavior = try c.decodeIfPresent(AppBehaviorKind.self, forKey: .behavior) ?? .default
    }
}

public typealias AppRule = AppRuleItem

public struct ConfigProfile: Codable, Equatable, Sendable, Identifiable {
    public var name: String = "Mặc định"
    public var modernTone: Bool = true
    public var freeTone: Bool = true
    public var allowFjwz: Bool = true
    public var autoCapitalizeSentence: Bool = false
    public var spellingCheck: Bool = true
    public var autoRestoreWrongWord: Bool = true
    public var smartCodeBypass: Bool = true
    public var macroEnabled: Bool = true
    public var macroWhenEnglish: Bool = false
    public var macroTriggerTabOnly: Bool = false
    public var quickWToUHorn: Bool = true
    public var quickBrackets: Bool = true
    public var quickTelex: Bool = false
    public var quickStartConsonant: Bool = false
    public var quickEndConsonant: Bool = false

    public var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name
        case modernTone = "modern_tone"
        case freeTone = "free_tone"
        case allowFjwz = "allow_fjwz"
        case autoCapitalizeSentence = "auto_capitalize_sentence"
        case spellingCheck = "spelling_check"
        case autoRestoreWrongWord = "auto_restore_wrong_word"
        case smartCodeBypass = "smart_code_bypass"
        case macroEnabled = "macro_enabled"
        case macroWhenEnglish = "macro_when_english"
        case macroTriggerTabOnly = "macro_trigger_tab_only"
        case quickWToUHorn = "quick_w_to_u_horn"
        case quickBrackets = "quick_brackets"
        case quickTelex = "quick_telex"
        case quickStartConsonant = "quick_start_consonant"
        case quickEndConsonant = "quick_end_consonant"
    }

    public init(
        name: String = "Mặc định",
        modernTone: Bool = true,
        freeTone: Bool = true,
        allowFjwz: Bool = true,
        autoCapitalizeSentence: Bool = false,
        spellingCheck: Bool = true,
        autoRestoreWrongWord: Bool = true,
        smartCodeBypass: Bool = true,
        macroEnabled: Bool = true,
        macroWhenEnglish: Bool = false,
        macroTriggerTabOnly: Bool = false,
        quickWToUHorn: Bool = true,
        quickBrackets: Bool = true,
        quickTelex: Bool = false,
        quickStartConsonant: Bool = false,
        quickEndConsonant: Bool = false
    ) {
        self.name = name
        self.modernTone = modernTone
        self.freeTone = freeTone
        self.allowFjwz = allowFjwz
        self.autoCapitalizeSentence = autoCapitalizeSentence
        self.spellingCheck = spellingCheck
        self.autoRestoreWrongWord = autoRestoreWrongWord
        self.smartCodeBypass = smartCodeBypass
        self.macroEnabled = macroEnabled
        self.macroWhenEnglish = macroWhenEnglish
        self.macroTriggerTabOnly = macroTriggerTabOnly
        self.quickWToUHorn = quickWToUHorn
        self.quickBrackets = quickBrackets
        self.quickTelex = quickTelex
        self.quickStartConsonant = quickStartConsonant
        self.quickEndConsonant = quickEndConsonant
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = ConfigProfile()
        name = try c.decodeIfPresent(String.self, forKey: .name) ?? d.name
        modernTone = try c.decodeIfPresent(Bool.self, forKey: .modernTone) ?? d.modernTone
        freeTone = try c.decodeIfPresent(Bool.self, forKey: .freeTone) ?? d.freeTone
        allowFjwz = try c.decodeIfPresent(Bool.self, forKey: .allowFjwz) ?? d.allowFjwz
        autoCapitalizeSentence = try c.decodeIfPresent(Bool.self, forKey: .autoCapitalizeSentence) ?? d.autoCapitalizeSentence
        spellingCheck = try c.decodeIfPresent(Bool.self, forKey: .spellingCheck) ?? d.spellingCheck
        autoRestoreWrongWord = try c.decodeIfPresent(Bool.self, forKey: .autoRestoreWrongWord) ?? d.autoRestoreWrongWord
        smartCodeBypass = try c.decodeIfPresent(Bool.self, forKey: .smartCodeBypass) ?? d.smartCodeBypass
        macroEnabled = try c.decodeIfPresent(Bool.self, forKey: .macroEnabled) ?? d.macroEnabled
        macroWhenEnglish = try c.decodeIfPresent(Bool.self, forKey: .macroWhenEnglish) ?? d.macroWhenEnglish
        macroTriggerTabOnly = try c.decodeIfPresent(Bool.self, forKey: .macroTriggerTabOnly) ?? d.macroTriggerTabOnly
        quickWToUHorn = try c.decodeIfPresent(Bool.self, forKey: .quickWToUHorn) ?? d.quickWToUHorn
        quickBrackets = try c.decodeIfPresent(Bool.self, forKey: .quickBrackets) ?? d.quickBrackets
        quickTelex = try c.decodeIfPresent(Bool.self, forKey: .quickTelex) ?? d.quickTelex
        quickStartConsonant = try c.decodeIfPresent(Bool.self, forKey: .quickStartConsonant) ?? d.quickStartConsonant
        quickEndConsonant = try c.decodeIfPresent(Bool.self, forKey: .quickEndConsonant) ?? d.quickEndConsonant
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
    public var show_switch_osd: Bool = false
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
    public var smartCodeBypass: Bool = true
    public var macroTriggerTabOnly: Bool = false
    public var holdEnglishKey: HoldEnglishKey = .none
    public var quickCapslockSwitch: Bool = false
    public var undoLastWordHotkey: HotkeyComboModel = HotkeyComboModel(ctrl: true, alt: false, shift: true, win_cmd: false, key_code: 0x5A)
    public var convertSelectionHotkey: HotkeyComboModel = HotkeyComboModel(ctrl: true, alt: false, shift: true, win_cmd: false, key_code: 0x76)
    public var showCaretOsd: Bool = false
    public var reeditSurroundingWord: Bool = true
    public var checkUpdatesOnStart: Bool = false
    public var activeProfile: String = "Mặc định"
    public var profiles: [ConfigProfile] = []
    public var syncConfigPath: String = ""
    public var configVersion: Int = MSKeyConfig.currentConfigVersion

    public static let currentConfigVersion = 1

    enum CodingKeys: String, CodingKey {
        case vietnamese_enabled
        case charset
        case input_method
        case mechanism
        case modern_tone
        case free_tone
        case allow_fjwz
        case auto_capitalize_sentence
        case use_clipboard_send
        case standard_key_send
        case spelling_check
        case auto_restore_wrong_word
        case disable_on_non_us_layout
        case fix_browser_suggest_1
        case fix_browser_suggest_2
        case macro_enabled
        case macro_when_english
        case quick_w_to_u_horn
        case quick_brackets
        case quick_telex
        case quick_start_consonant
        case quick_end_consonant
        case esc_restore
        case switch_preset
        case custom_switch_hotkey
        case clipboard_convert_hotkey
        case beep_on_switch
        case show_switch_osd
        case paste_via_shift_insert
        case send_delay_ms
        case auto_start
        case run_as_admin
        case show_on_startup
        case theme
        case smart_app_switch
        case app_rules
        case macros
        case user_dict_words
        case custom_keymap
        case smartCodeBypass = "smart_code_bypass"
        case macroTriggerTabOnly = "macro_trigger_tab_only"
        case holdEnglishKey = "hold_english_key"
        case quickCapslockSwitch = "quick_capslock_switch"
        case undoLastWordHotkey = "undo_last_word_hotkey"
        case convertSelectionHotkey = "convert_selection_hotkey"
        case showCaretOsd = "show_caret_osd"
        case reeditSurroundingWord = "reedit_surrounding_word"
        case checkUpdatesOnStart = "check_updates_on_start"
        case activeProfile = "active_profile"
        case profiles
        case syncConfigPath = "sync_config_path"
        case configVersion = "config_version"
    }

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
        smartCodeBypass = try c.decodeIfPresent(Bool.self, forKey: .smartCodeBypass) ?? d.smartCodeBypass
        macroTriggerTabOnly = try c.decodeIfPresent(Bool.self, forKey: .macroTriggerTabOnly) ?? d.macroTriggerTabOnly
        holdEnglishKey = try c.decodeIfPresent(HoldEnglishKey.self, forKey: .holdEnglishKey) ?? d.holdEnglishKey
        quickCapslockSwitch = try c.decodeIfPresent(Bool.self, forKey: .quickCapslockSwitch) ?? d.quickCapslockSwitch
        undoLastWordHotkey = try c.decodeIfPresent(HotkeyComboModel.self, forKey: .undoLastWordHotkey) ?? d.undoLastWordHotkey
        convertSelectionHotkey = try c.decodeIfPresent(HotkeyComboModel.self, forKey: .convertSelectionHotkey) ?? d.convertSelectionHotkey
        showCaretOsd = try c.decodeIfPresent(Bool.self, forKey: .showCaretOsd) ?? d.showCaretOsd
        reeditSurroundingWord = try c.decodeIfPresent(Bool.self, forKey: .reeditSurroundingWord) ?? d.reeditSurroundingWord
        checkUpdatesOnStart = try c.decodeIfPresent(Bool.self, forKey: .checkUpdatesOnStart) ?? d.checkUpdatesOnStart
        activeProfile = try c.decodeIfPresent(String.self, forKey: .activeProfile) ?? d.activeProfile
        profiles = try c.decodeIfPresent([ConfigProfile].self, forKey: .profiles) ?? d.profiles
        syncConfigPath = try c.decodeIfPresent(String.self, forKey: .syncConfigPath) ?? d.syncConfigPath
        configVersion = try c.decodeIfPresent(Int.self, forKey: .configVersion) ?? 0
        if configVersion < 1 {
            show_switch_osd = false
            showCaretOsd = false
        }
        configVersion = max(configVersion, MSKeyConfig.currentConfigVersion)
    }

    public static let builtinProfileNames = ["Mặc định", "Lập trình", "Soạn thảo"]

    public func allProfileNames() -> [String] {
        var names = Self.builtinProfileNames
        for p in profiles {
            let trimmed = p.name.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty && !names.contains(where: { $0.caseInsensitiveCompare(trimmed) == .orderedSame }) {
                names.append(trimmed)
            }
        }
        return names
    }

    public func snapshotProfile(named name: String) -> ConfigProfile {
        ConfigProfile(
            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
            modernTone: modern_tone,
            freeTone: free_tone,
            allowFjwz: allow_fjwz,
            autoCapitalizeSentence: auto_capitalize_sentence,
            spellingCheck: spelling_check,
            autoRestoreWrongWord: auto_restore_wrong_word,
            smartCodeBypass: smartCodeBypass,
            macroEnabled: macro_enabled,
            macroWhenEnglish: macro_when_english,
            macroTriggerTabOnly: macroTriggerTabOnly,
            quickWToUHorn: quick_w_to_u_horn,
            quickBrackets: quick_brackets,
            quickTelex: quick_telex,
            quickStartConsonant: quick_start_consonant,
            quickEndConsonant: quick_end_consonant
        )
    }

    public mutating func saveCurrentAsProfile(_ name: String) {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return }
        let snap = snapshotProfile(named: clean)
        if let idx = profiles.firstIndex(where: { $0.name.caseInsensitiveCompare(clean) == .orderedSame }) {
            profiles[idx] = snap
        } else {
            profiles.append(snap)
        }
        activeProfile = clean
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

    public static func resolveSyncURL(from rawPath: String) -> URL? {
        let trimmed = rawPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let expanded = NSString(string: trimmed).expandingTildeInPath
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: expanded, isDirectory: &isDir), isDir.boolValue {
            return URL(fileURLWithPath: expanded, isDirectory: true).appendingPathComponent("config.json")
        }
        if expanded.hasSuffix("/") {
            return URL(fileURLWithPath: expanded, isDirectory: true).appendingPathComponent("config.json")
        }
        return URL(fileURLWithPath: expanded, isDirectory: false)
    }

    public static func load() -> MSKeyConfig {
        load(from: configFile)
    }

    public static func load(from primaryFile: URL) -> MSKeyConfig {
        let localCfg: MSKeyConfig
        let localExists: Bool
        if let data = try? Data(contentsOf: primaryFile),
           let cfg = try? JSONDecoder().decode(MSKeyConfig.self, from: data) {
            localCfg = cfg
            localExists = true
        } else {
            localCfg = MSKeyConfig()
            localExists = false
        }
        if let syncURL = resolveSyncURL(from: localCfg.syncConfigPath),
           syncURL.standardizedFileURL != primaryFile.standardizedFileURL,
           let syncData = try? Data(contentsOf: syncURL),
           var syncCfg = try? JSONDecoder().decode(MSKeyConfig.self, from: syncData) {
            let localMod = (try? FileManager.default.attributesOfItem(atPath: primaryFile.path)[.modificationDate] as? Date) ?? .distantPast
            let syncMod = (try? FileManager.default.attributesOfItem(atPath: syncURL.path)[.modificationDate] as? Date) ?? .distantPast
            if !localExists || syncMod > localMod {
                syncCfg.syncConfigPath = localCfg.syncConfigPath
                writeConfigData(syncCfg, to: primaryFile)
                return syncCfg
            }
        }
        return localCfg
    }

    public static func save(_ config: MSKeyConfig) {
        save(config, to: configFile)
    }

    public static func save(_ config: MSKeyConfig, to primaryFile: URL) {
        writeConfigData(config, to: primaryFile)
        if let syncURL = resolveSyncURL(from: config.syncConfigPath),
           syncURL.standardizedFileURL != primaryFile.standardizedFileURL {
            writeConfigData(config, to: syncURL)
        }
    }

    private static func writeConfigData(_ config: MSKeyConfig, to fileURL: URL) {
        try? FileManager.default.createDirectory(at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(config) {
            try? data.write(to: fileURL, options: .atomic)
        }
    }

    public static func formatUserDictText(_ words: [String]) -> String {
        var out = "# MS Key User Dictionary\n"
        for word in words {
            let t = word.trimmingCharacters(in: .whitespacesAndNewlines)
            if !t.isEmpty {
                out += t + "\n"
            }
        }
        return out
    }

    public static func parseUserDictText(_ content: String) -> [String] {
        var seen = Set<String>()
        var words: [String] = []
        for rawLine in content.components(separatedBy: .newlines) {
            let line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty || line.hasPrefix("#") || line.hasPrefix(";") {
                continue
            }
            if seen.insert(line.lowercased()).inserted {
                words.append(line)
            }
        }
        return words
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
