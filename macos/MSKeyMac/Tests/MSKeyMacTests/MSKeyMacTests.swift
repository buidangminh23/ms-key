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

    func testRustBridgeTypingEnglishRestoreAndEscape() {
        let engine = RustEngine()
        XCTAssertTrue(engine.isEnabled())
        XCTAssertEqual(typeTelex(engine, "tieengs Vieejt "), "tiếng Việt ")
        XCTAssertEqual(typeTelex(engine, "tesst "), "tesst ")

        let _ = typeTelex(engine, "vieejt")
        let esc = engine.processEscape()
        XCTAssertTrue(esc.replace)
        XCTAssertEqual(esc.text, "vieejt")
    }

    func testConfigJSONRoundTripAndCharsetConvert() {
        var cfg = MSKeyConfig()
        cfg.input_method = .vni
        cfg.charset = .tcvn3
        cfg.theme = .dark
        cfg.app_rules = [AppRuleItem(process_name: "Terminal", behavior: .forceEnglish)]
        cfg.macros = [MacroItem(shortcut: "vn", expansion: "Việt Nam", enabled: true)]

        let enc = JSONEncoder()
        let data = try? enc.encode(cfg)
        XCTAssertNotNil(data)
        let json = data.flatMap { String(data: $0, encoding: .utf8) } ?? "{}"

        let engine = RustEngine(configJSON: json)
        XCTAssertEqual(engine.behavior(for: "Terminal"), .forceEnglish)
        XCTAssertEqual(engine.behavior(for: "Notes"), .default)

        let round = engine.getConfigJSON()
        let decoded = round.data(using: .utf8).flatMap { try? JSONDecoder().decode(MSKeyConfig.self, from: $0) }
        XCTAssertEqual(decoded?.input_method, .vni)
        XCTAssertEqual(decoded?.theme, .dark)
        XCTAssertEqual(decoded?.macros.first?.expansion, "Việt Nam")

        let tcvn3 = RustEngine.convertText("Tiếng Việt", from: .unicode, to: .tcvn3)
        let back = RustEngine.convertText(tcvn3, from: .tcvn3, to: .unicode)
        XCTAssertEqual(back, "Tiếng Việt")
    }

    func testHotkeyTrackerSmartMemoryAndMacKeycode() {
        var tracker = MacHotkeyTracker()
        let conv = HotkeyComboModel()
        let custom = HotkeyComboModel(ctrl: true, alt: true, shift: false, win_cmd: false, key_code: 0x56)

        let ctrlShiftFlags = UInt64(CGEventFlags.maskControl.rawValue | CGEventFlags.maskShift.rawValue)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .ctrlShift, flags: ctrlShiftFlags), .none)
        XCTAssertEqual(tracker.handleFlagsChanged(preset: .ctrlShift, flags: 0), .toggleVE)

        let altMask = UInt64(CGEventFlags.maskAlternate.rawValue)
        XCTAssertEqual(
            tracker.handleKeyDown(preset: .altZ, customSwitch: custom, convertHotkey: conv, vk: 0x5A, flags: altMask),
            .toggleVE
        )
        XCTAssertEqual(
            tracker.handleKeyDown(preset: .ctrlShift, customSwitch: custom, convertHotkey: conv, vk: 0x75, flags: ctrlShiftFlags),
            .convertClipboard
        )

        XCTAssertEqual(MacKeycode.toPortableVK(0x06), 0x5A)
        XCTAssertEqual(MacKeycode.toPortableVK(0x61), 0x75)

        var mem = SmartAppSessionMemory(initialVE: true)
        XCTAssertEqual(mem.switchApp("Terminal", behavior: .forceEnglish, smartSwitch: true), false)
        XCTAssertEqual(mem.switchApp("Notes", behavior: .default, smartSwitch: true), true)
    }
}
