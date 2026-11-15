import CMSKeyCore
import Foundation

public struct KeyActionResult: Equatable, Sendable {
    public let replace: Bool
    public let backspaces: Int
    public let text: String

    public static let passThrough = KeyActionResult(replace: false, backspaces: 0, text: "")

    init(from cAction: MsKeyAction) {
        self.replace = cAction.kind == 1
        self.backspaces = Int(cAction.backspaces)
        if let ptr = cAction.text {
            self.text = String(cString: ptr)
        } else {
            self.text = ""
        }
    }

    public init(replace: Bool, backspaces: Int, text: String) {
        self.replace = replace
        self.backspaces = backspaces
        self.text = text
    }
}

public final class RustEngine: @unchecked Sendable {
    private let lock = NSLock()
    private var handle: OpaquePointer?

    public init(configJSON: String? = nil) {
        if let json = configJSON {
            self.handle = json.withCString { mskey_create($0) }
        } else {
            self.handle = mskey_create(nil)
        }
        syncClockOffset()
    }

    deinit {
        lock.lock()
        let h = handle
        handle = nil
        lock.unlock()
        if let h {
            mskey_destroy(h)
        }
    }

    public static func defaultConfigJSON() -> String {
        guard let ptr = mskey_default_config_json() else { return "{}" }
        defer { mskey_free_string(ptr) }
        return String(cString: ptr)
    }

    public func getConfigJSON() -> String {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle, let ptr = mskey_get_config_json(h) else { return "{}" }
        defer { mskey_free_string(ptr) }
        return String(cString: ptr)
    }

    @discardableResult
    public func setConfig(_ config: MSKeyConfig) -> Bool {
        let enc = JSONEncoder()
        guard let data = try? enc.encode(config),
              let json = String(data: data, encoding: .utf8) else {
            return false
        }
        return setConfigJSON(json)
    }

    @discardableResult
    public func setConfigJSON(_ json: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return false }
        let ok = json.withCString { mskey_set_config_json(h, $0) }
        mskey_set_clock_offset(h, Int64(TimeZone.current.secondsFromGMT()))
        return ok
    }

    public func syncClockOffset() {
        lock.lock()
        defer { lock.unlock() }
        if let h = handle {
            mskey_set_clock_offset(h, Int64(TimeZone.current.secondsFromGMT()))
        }
    }

    public func isActiveDelimiter(scalar: UInt32) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return false }
        return mskey_is_active_delimiter(h, scalar)
    }

    public func setEnabled(_ enabled: Bool) {
        lock.lock()
        defer { lock.unlock() }
        if let h = handle {
            mskey_set_enabled(h, enabled)
        }
    }

    public func isEnabled() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return false }
        return mskey_is_enabled(h)
    }

    public func resetWord() {
        lock.lock()
        defer { lock.unlock() }
        if let h = handle {
            mskey_reset_word(h)
        }
    }

    public func resetAll() {
        lock.lock()
        defer { lock.unlock() }
        if let h = handle {
            mskey_reset_all(h)
        }
    }

    public func processKey(scalar: UInt32) -> KeyActionResult {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return .passThrough }
        let act = mskey_process_key(h, scalar)
        let res = KeyActionResult(from: act)
        mskey_free_action(act)
        return res
    }

    public func processBackspace() -> KeyActionResult {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return .passThrough }
        let act = mskey_process_backspace(h)
        let res = KeyActionResult(from: act)
        mskey_free_action(act)
        return res
    }

    public func processEscape() -> KeyActionResult {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return .passThrough }
        let act = mskey_process_escape(h)
        let res = KeyActionResult(from: act)
        mskey_free_action(act)
        return res
    }

    public func behavior(for appName: String) -> AppBehaviorKind {
        lock.lock()
        defer { lock.unlock() }
        guard let h = handle else { return .default }
        let code = appName.withCString { mskey_behavior_for_app(h, $0) }
        switch code {
        case 1: return .forceEnglish
        case 2: return .forceVietnamese
        case 3: return .blacklist
        default: return .default
        }
    }

    public static func convertText(_ input: String, from: CharsetKind, to: CharsetKind, caseMode: UInt8 = 0) -> String {
        input.withCString { ptr in
            guard let out = mskey_convert_text(ptr, from.ffiCode, to.ffiCode, caseMode) else {
                return input
            }
            defer { mskey_free_string(out) }
            return String(cString: out)
        }
    }

    public static func detectCharset(_ input: String) -> ConvertEncodingKind {
        let code = input.withCString { mskey_detect_charset($0) }
        return ConvertEncodingKind(rawValue: code) ?? .unicode
    }

    public static func convertWithEncoding(
        _ input: String,
        from: ConvertEncodingKind,
        to: ConvertEncodingKind,
        caseMode: UInt8 = 0
    ) -> String {
        let src = from == .auto ? detectCharset(input) : from
        let dst = to == .auto ? .unicode : to
        return input.withCString { ptr in
            guard let out = mskey_convert_with_encoding(ptr, src.rawValue, dst.rawValue, caseMode) else {
                return input
            }
            defer { mskey_free_string(out) }
            return String(cString: out)
        }
    }

    public static func formatMacroText(config: MSKeyConfig) -> String {
        let enc = JSONEncoder()
        guard let data = try? enc.encode(config),
              let json = String(data: data, encoding: .utf8) else {
            return ""
        }
        return json.withCString { ptr in
            guard let out = mskey_format_macro_text(ptr) else { return "" }
            defer { mskey_free_string(out) }
            return String(cString: out)
        }
    }

    public static func parseMacroText(_ rawText: String) -> [MacroItem] {
        rawText.withCString { ptr in
            guard let out = mskey_parse_macro_text(ptr) else { return [] }
            defer { mskey_free_string(out) }
            let jsonStr = String(cString: out)
            guard let data = jsonStr.data(using: .utf8) else { return [] }
            return (try? JSONDecoder().decode([MacroItem].self, from: data)) ?? []
        }
    }

    public static func importEVKeyTree(_ rawText: String) -> MSKeyConfig? {
        rawText.withCString { ptr in
            guard let jsonPtr = mskey_import_evkey_tree(ptr) else { return nil }
            defer { mskey_free_string(jsonPtr) }
            let jsonStr = String(cString: jsonPtr)
            guard let data = jsonStr.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(MSKeyConfig.self, from: data)
        }
    }
}
