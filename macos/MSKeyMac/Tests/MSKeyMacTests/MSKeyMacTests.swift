import ApplicationServices
import Carbon
import Foundation
import XCTest
@testable import MSKeyKit

final class MSKeyMacTests: XCTestCase {
    private func typeTelex(_ engine: RustEngine, _ input: String) -> String {
        var out = ""
        for scalar in input.unicodeScalars {
            let res = engine.processKey(scalar: scalar.value)
            if res.replace {
                for _ in 0..<res.backspaces where !out.isEmpty {
                    out.removeLast()
                }
                out.append(res.text)
            } else {
                out.append(Character(scalar))
            }
        }
        return out
    }

    func testRustBridgeTypingWrongWordRestoreAndEscape() {
        let engine = RustEngine()
        engine.syncClockOffset()
        XCTAssertTrue(engine.isEnabled())
        XCTAssertTrue(engine.isActiveDelimiter(scalar: 0x20))
        XCTAssertFalse(engine.isActiveDelimiter(scalar: 0x61))

        XCTAssertEqual(typeTelex(engine, "tieengs Vieejt "), "tiếng Việt ")
        XCTAssertEqual(typeTelex(engine, "window "), "window ")

        let _ = typeTelex(engine, "vieejt")
        let esc = engine.processEscape()
        XCTAssertTrue(esc.replace)
        XCTAssertEqual(esc.text, "vieejt")
    }

    func testConfigJSONRoundTripBackwardCompatAndCharsetConvert() {
        var cfg = MSKeyConfig()
        cfg.input_method = .vni
        cfg.charset = .tcvn3
        cfg.theme = .dark
        cfg.quick_telex = true
        cfg.show_switch_osd = false
        cfg.send_delay_ms = 5
        cfg.app_rules = [AppRuleItem(process_name: "Terminal", behavior: .forceEnglish)]
        cfg.macros = [MacroItem(shortcut: "vn", expansion: "Việt Nam\n2026", enabled: true)]

        let data = try? JSONEncoder().encode(cfg)
        XCTAssertNotNil(data)
        let json = data.flatMap { String(data: $0, encoding: .utf8) } ?? "{}"

        let engine = RustEngine(configJSON: json)
        XCTAssertEqual(engine.behavior(for: "Terminal"), .forceEnglish)
        XCTAssertEqual(engine.behavior(for: "Notes"), .default)

        let round = engine.getConfigJSON()
        let decoded = round.data(using: .utf8).flatMap { try? JSONDecoder().decode(MSKeyConfig.self, from: $0) }
        XCTAssertEqual(decoded?.input_method, .vni)
        XCTAssertEqual(decoded?.theme, .dark)
        XCTAssertEqual(decoded?.quick_telex, true)
        XCTAssertEqual(decoded?.send_delay_ms, 5)
        XCTAssertEqual(decoded?.macros.first?.expansion, "Việt Nam\n2026")

        let legacyJSON = "{\"vietnamese_enabled\":false,\"custom_keymap\":{\"acute\":\"s\"}}"
        let legacyDecoded = legacyJSON.data(using: .utf8).flatMap { try? JSONDecoder().decode(MSKeyConfig.self, from: $0) }
        XCTAssertEqual(legacyDecoded?.vietnamese_enabled, false)
        XCTAssertEqual(legacyDecoded?.show_switch_osd, true)
        XCTAssertEqual(legacyDecoded?.send_delay_ms, 2)
        XCTAssertEqual(legacyDecoded?.auto_start, true)
        XCTAssertEqual(legacyDecoded?.show_on_startup, false)
        XCTAssertEqual(legacyDecoded?.custom_keymap.quick_u_horn, "w")

        let tcvn3 = RustEngine.convertWithEncoding("Tiếng Việt", from: .unicode, to: .tcvn3)
        XCTAssertEqual(RustEngine.detectCharset(tcvn3), .tcvn3)
        XCTAssertEqual(RustEngine.convertWithEncoding(tcvn3, from: .auto, to: .unicode), "Tiếng Việt")

        let viqr = RustEngine.convertWithEncoding("Tiếng Việt", from: .unicode, to: .viqr)
        XCTAssertEqual(RustEngine.convertWithEncoding(viqr, from: .viqr, to: .unicode), "Tiếng Việt")
        XCTAssertEqual(RustEngine.convertWithEncoding("Tiếng Việt", from: .unicode, to: .unicode, caseMode: 5), "Tieng Viet")

        let macroText = RustEngine.formatMacroText(config: cfg)
        let parsedMacros = RustEngine.parseMacroText(macroText)
        XCTAssertEqual(parsedMacros.first?.shortcut, "vn")
        XCTAssertEqual(parsedMacros.first?.expansion, "Việt Nam\n2026")
    }

    func testHotkeyTrackerPresetsPunctuationAndSmartMemory() {
        var tracker = MacHotkeyTracker()
        let conv = HotkeyComboModel()
        let custom = HotkeyComboModel(ctrl: true, alt: true, shift: false, win_cmd: false, key_code: 0x56)

        let ctrlShiftFlags = UInt64(CGEventFlags.maskControl.rawValue | CGEventFlags.maskShift.rawValue)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .ctrlShift, flags: ctrlShiftFlags, now: 1.0), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .ctrlShift, flags: 0, now: 1.1), .toggleVE)

        let ctrlMask = UInt64(CGEventFlags.maskControl.rawValue)
        XCTAssertEqual(
            tracker.handleKeyDown(preset: .ctrlSpace, customSwitch: custom, convertHotkey: conv, vk: 0x20, flags: ctrlMask),
            .toggleVE
        )

        let shiftMask = UInt64(CGEventFlags.maskShift.rawValue)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: shiftMask, now: 2.0), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: 0, now: 2.1), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: shiftMask, now: 2.2), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: 0, now: 2.3), .toggleVE)

        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: shiftMask, now: 5.0), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: 0, now: 5.6), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: shiftMask, now: 5.7), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .shiftShift, flags: 0, now: 5.8), .none)

        let altMask = UInt64(CGEventFlags.maskAlternate.rawValue)
        XCTAssertEqual(
            tracker.handleKeyDown(preset: .altZ, customSwitch: custom, convertHotkey: conv, vk: 0x5A, flags: altMask),
            .toggleVE
        )
        XCTAssertEqual(
            tracker.handleKeyDown(preset: .ctrlShift, customSwitch: custom, convertHotkey: conv, vk: 0x75, flags: ctrlShiftFlags),
            .convertClipboard
        )

        XCTAssertEqual(MacKeycode.toPortableVK(UInt16(kVK_ANSI_Z)), 0x5A)
        XCTAssertEqual(MacKeycode.toPortableVK(UInt16(kVK_F6)), 0x75)
        XCTAssertEqual(MacKeycode.toPortableVK(UInt16(kVK_ANSI_LeftBracket)), 0xDB)
        XCTAssertEqual(MacKeycode.toPortableVK(UInt16(kVK_ANSI_Semicolon)), 0xBA)

        var mem = SmartAppSessionMemory(initialVE: true)
        mem.loadMemory(["Notes": false])
        XCTAssertEqual(mem.switchApp("Terminal", behavior: .forceEnglish, smartSwitch: true), false)
        XCTAssertEqual(mem.switchApp("Notes", behavior: .default, smartSwitch: true), false)
        XCTAssertEqual(mem.exportMemory()["notes"], false)
    }
}
