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
        XCTAssertEqual(legacyDecoded?.show_switch_osd, false)
        XCTAssertEqual(legacyDecoded?.showCaretOsd, false)
        XCTAssertEqual(legacyDecoded?.send_delay_ms, 2)
        XCTAssertEqual(legacyDecoded?.auto_start, true)
        XCTAssertEqual(legacyDecoded?.show_on_startup, false)
        XCTAssertEqual(legacyDecoded?.custom_keymap.quick_u_horn, "w")

        let oldOsdJSON = "{\"show_switch_osd\":true,\"show_caret_osd\":true}"
        let oldOsd = oldOsdJSON.data(using: .utf8).flatMap { try? JSONDecoder().decode(MSKeyConfig.self, from: $0) }
        XCTAssertEqual(oldOsd?.show_switch_osd, false)
        XCTAssertEqual(oldOsd?.showCaretOsd, false)
        XCTAssertEqual(oldOsd?.configVersion, 1)
        let chosenOsdJSON = "{\"show_switch_osd\":true,\"show_caret_osd\":true,\"config_version\":1}"
        let chosenOsd = chosenOsdJSON.data(using: .utf8).flatMap { try? JSONDecoder().decode(MSKeyConfig.self, from: $0) }
        XCTAssertEqual(chosenOsd?.show_switch_osd, true)
        XCTAssertEqual(chosenOsd?.showCaretOsd, true)

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

        var session = AppRuleSession(initialVE: true)
        XCTAssertEqual(session.switchApp("Terminal", behavior: .forceEnglish), false)
        XCTAssertEqual(session.switchApp("Notes", behavior: .default), true)
        XCTAssertNil(session.switchApp("Notes", behavior: .default))
        session.setUserVE(false)
        XCTAssertEqual(session.switchApp("Claude", behavior: .default), false)
        session.setUserVE(true)
        session.invalidateActive()
        XCTAssertEqual(session.switchApp("Claude", behavior: .default), true)
        XCTAssertEqual(session.switchApp("Claude", behavior: .forceEnglish), false)
        XCTAssertEqual(session.switchApp("Claude", behavior: .default), true)

        let rightShiftFlags = UInt64(CGEventFlags.maskShift.rawValue) | 0x0004
        let leftShiftFlags = UInt64(CGEventFlags.maskShift.rawValue) | 0x0002
        XCTAssertEqual(
            tracker.updateHoldEnglish(holdKey: .rightShift, flags: leftShiftFlags, keyCode: UInt16(kVK_Shift)),
            nil
        )
        XCTAssertFalse(tracker.holdEnglishActive)
        XCTAssertEqual(
            tracker.updateHoldEnglish(holdKey: .rightShift, flags: rightShiftFlags, keyCode: UInt16(kVK_RightShift)),
            true
        )
        XCTAssertEqual(
            tracker.updateHoldEnglish(holdKey: .rightShift, flags: 0, keyCode: UInt16(kVK_RightShift)),
            false
        )

        var capsTracker = MacHotkeyTracker()
        let capsMask = UInt64(CGEventFlags.maskAlphaShift.rawValue)
        XCTAssertEqual(
            capsTracker.handleFlagsChanged(
                preset: .ctrlShift,
                flags: capsMask,
                now: 1.0,
                quickCapslockSwitch: true,
                keyCode: UInt16(kVK_CapsLock)
            ),
            .toggleVE
        )
        XCTAssertEqual(
            capsTracker.handleFlagsChanged(
                preset: .ctrlShift,
                flags: capsMask,
                now: 1.1,
                quickCapslockSwitch: true,
                keyCode: UInt16(kVK_CapsLock)
            ),
            .none
        )

        let undoCombo = HotkeyComboModel(ctrl: true, alt: false, shift: true, win_cmd: false, key_code: 0x5A)
        let selCombo = HotkeyComboModel(ctrl: true, alt: false, shift: true, win_cmd: false, key_code: 0x76)
        XCTAssertEqual(
            tracker.handleKeyDown(
                preset: .ctrlShift,
                customSwitch: custom,
                convertHotkey: conv,
                vk: 0x5A,
                flags: ctrlShiftFlags,
                undoHotkey: undoCombo,
                convertSelectionHotkey: selCombo
            ),
            .undoLastWord
        )
        XCTAssertEqual(
            tracker.handleKeyDown(
                preset: .ctrlShift,
                customSwitch: custom,
                convertHotkey: conv,
                vk: 0x76,
                flags: ctrlShiftFlags,
                undoHotkey: undoCombo,
                convertSelectionHotkey: selCombo
            ),
            .convertSelection
        )
    }

    func testNewConfigFieldsProfilesWindowRulesAndSyncStore() throws {
        var cfg = MSKeyConfig()
        cfg.smartCodeBypass = false
        cfg.macroTriggerTabOnly = true
        cfg.holdEnglishKey = .rightShift
        cfg.quickCapslockSwitch = true
        cfg.undoLastWordHotkey = HotkeyComboModel(ctrl: true, alt: true, shift: false, win_cmd: false, key_code: 0x5A)
        cfg.convertSelectionHotkey = HotkeyComboModel(ctrl: true, alt: false, shift: true, win_cmd: false, key_code: 0x77)
        cfg.showCaretOsd = true
        cfg.reeditSurroundingWord = false
        cfg.checkUpdatesOnStart = true
        cfg.activeProfile = "CustomDev"
        cfg.syncConfigPath = "/tmp/mskey-sync-test.json"
        cfg.app_rules = [
            AppRule(process_name: "Code", titlePattern: "*terminal*", behavior: .forceEnglish),
            AppRule(process_name: "Code", titlePattern: "", behavior: .forceVietnamese),
        ]
        cfg.saveCurrentAsProfile("CustomDev")

        let data = try JSONEncoder().encode(cfg)
        let json = try XCTUnwrap(String(data: data, encoding: .utf8))
        XCTAssertTrue(json.contains("\"hold_english_key\":\"RightShift\""))
        XCTAssertTrue(json.contains("\"title_pattern\":\"*terminal*\""))
        XCTAssertTrue(json.contains("\"smart_code_bypass\":false"))
        XCTAssertTrue(json.contains("\"macro_trigger_tab_only\":true"))
        XCTAssertTrue(json.contains("\"quick_capslock_switch\":true"))
        XCTAssertTrue(json.contains("\"show_caret_osd\":true"))
        XCTAssertTrue(json.contains("\"reedit_surrounding_word\":false"))
        XCTAssertTrue(json.contains("\"check_updates_on_start\":true"))
        XCTAssertTrue(json.contains("\"sync_config_path\":\"\\/tmp\\/mskey-sync-test.json\"") || json.contains("\"sync_config_path\":\"/tmp/mskey-sync-test.json\""))

        for variant in HoldEnglishKey.allCases {
            let encoded = try JSONEncoder().encode(variant)
            let decoded = try JSONDecoder().decode(HoldEnglishKey.self, from: encoded)
            XCTAssertEqual(decoded, variant)
        }

        let engine = RustEngine(configJSON: json)
        let rustRound = engine.getConfigJSON()
        let decoded = try JSONDecoder().decode(MSKeyConfig.self, from: XCTUnwrap(rustRound.data(using: .utf8)))
        XCTAssertEqual(decoded.smartCodeBypass, false)
        XCTAssertEqual(decoded.macroTriggerTabOnly, true)
        XCTAssertEqual(decoded.holdEnglishKey, .rightShift)
        XCTAssertEqual(decoded.quickCapslockSwitch, true)
        XCTAssertEqual(decoded.undoLastWordHotkey.key_code, 0x5A)
        XCTAssertEqual(decoded.convertSelectionHotkey.key_code, 0x77)
        XCTAssertEqual(decoded.showCaretOsd, true)
        XCTAssertEqual(decoded.reeditSurroundingWord, false)
        XCTAssertEqual(decoded.checkUpdatesOnStart, true)
        XCTAssertEqual(decoded.activeProfile, "CustomDev")
        XCTAssertEqual(decoded.profiles.count, 1)
        XCTAssertEqual(decoded.app_rules.first?.titlePattern, "*terminal*")
        XCTAssertEqual(decoded.syncConfigPath, "/tmp/mskey-sync-test.json")

        let legacy = try JSONDecoder().decode(MSKeyConfig.self, from: XCTUnwrap("{}".data(using: .utf8)))
        XCTAssertEqual(legacy.smartCodeBypass, true)
        XCTAssertEqual(legacy.macroTriggerTabOnly, false)
        XCTAssertEqual(legacy.holdEnglishKey, .none)
        XCTAssertEqual(legacy.quickCapslockSwitch, false)
        XCTAssertEqual(legacy.showCaretOsd, false)
        XCTAssertEqual(legacy.reeditSurroundingWord, true)
        XCTAssertEqual(legacy.checkUpdatesOnStart, false)
        XCTAssertEqual(legacy.activeProfile, "Mặc định")

        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("mskey-sync-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let primaryFile = tempDir.appendingPathComponent("config.json")
        let mirrorFile = tempDir.appendingPathComponent("mirror.json")
        var localCfg = MSKeyConfig()
        localCfg.syncConfigPath = mirrorFile.path
        localCfg.modern_tone = true
        MacConfigStore.save(localCfg, to: primaryFile)
        XCTAssertTrue(FileManager.default.fileExists(atPath: mirrorFile.path))

        var remoteCfg = localCfg
        remoteCfg.modern_tone = false
        remoteCfg.syncConfigPath = "/another/path.json"
        let remoteData = try JSONEncoder().encode(remoteCfg)
        try remoteData.write(to: mirrorFile, options: .atomic)
        let futureDate = Date().addingTimeInterval(30)
        try FileManager.default.setAttributes([.modificationDate: futureDate], ofItemAtPath: mirrorFile.path)

        let synced = MacConfigStore.load(from: primaryFile)
        XCTAssertEqual(synced.modern_tone, false)
        XCTAssertEqual(synced.syncConfigPath, mirrorFile.path)

        let formattedDict = MacConfigStore.formatUserDictText(["Swinburne", "Anthropic", "Swinburne"])
        XCTAssertEqual(MacConfigStore.parseUserDictText(formattedDict), ["Swinburne", "Anthropic"])
    }

    func testNewRustBridgeFeatureAPIs() throws {
        XCTAssertEqual(RustEngine.normalizeToneStyle("hòa khóa", modernTone: true), "hoà khoá")
        XCTAssertEqual(RustEngine.normalizeToneStyle("hoà khoá", modernTone: false), "hòa khóa")
        XCTAssertEqual(
            RustEngine.convertWithEncoding("hòa", from: .unicode, to: .unicode, caseTransform: .normalizeModernTone),
            "hoà"
        )
        XCTAssertEqual(
            RustEngine.convertWithEncoding("hoà", from: .unicode, to: .unicode, caseTransform: .normalizeClassicTone),
            "hòa"
        )

        var cfg = MSKeyConfig()
        cfg.app_rules = [
            AppRule(process_name: "Code", titlePattern: "*terminal*", behavior: .forceEnglish),
            AppRule(process_name: "Code", titlePattern: "", behavior: .forceVietnamese),
        ]
        cfg.macros = [
            MacroItem(shortcut: "cb", expansion: "Hi {clipboard}", enabled: true),
            MacroItem(shortcut: "tag", expansion: "<b>{cursor}</b>", enabled: true),
        ]
        let jsonData = try JSONEncoder().encode(cfg)
        let jsonStr = try XCTUnwrap(String(data: jsonData, encoding: .utf8))
        let engine = RustEngine(configJSON: jsonStr)

        XCTAssertEqual(engine.behaviorForWindow(processName: "Code", windowTitle: "zsh - Terminal"), .forceEnglish)
        XCTAssertEqual(engine.behaviorForWindow(processName: "Code", windowTitle: "App.swift"), .forceVietnamese)
        XCTAssertEqual(engine.behaviorForWindow(processName: "Notes", windowTitle: ""), .default)

        XCTAssertFalse(engine.toggleLastCommittedWord().replace)
        XCTAssertEqual(typeTelex(engine, "vieejt "), "việt ")
        let undoRes = engine.toggleLastCommittedWord()
        XCTAssertTrue(undoRes.replace)
        XCTAssertEqual(undoRes.text, "vieejt ")
        let redoRes = engine.toggleLastCommittedWord()
        XCTAssertTrue(redoRes.replace)
        XCTAssertEqual(redoRes.text, "việt ")

        engine.setTemporaryEnglish(true)
        XCTAssertEqual(typeTelex(engine, "vieetj "), "vieetj ")
        engine.setTemporaryEnglish(false)
        XCTAssertEqual(typeTelex(engine, "vieetj "), "việt ")

        engine.resetWord()
        engine.hydrateWord("xin tiếng")
        let hydratedEdit = engine.processKey(scalar: UnicodeScalar("f").value)
        XCTAssertTrue(hydratedEdit.replace)
        var screen = Array("xin tiếng")
        screen.removeLast(Int(hydratedEdit.backspaces))
        screen.append(contentsOf: hydratedEdit.text)
        XCTAssertEqual(String(screen), "xin tiềng")

        engine.resetWord()
        engine.setClipboardText("Swift6")
        XCTAssertEqual(typeTelex(engine, "cb "), "Hi Swift6 ")
        XCTAssertEqual(engine.takePendingCursorLeft(), 0)
        XCTAssertEqual(typeTelex(engine, "tag "), "<b></b> ")
        XCTAssertGreaterThan(engine.takePendingCursorLeft(), 0)
        XCTAssertEqual(engine.takePendingCursorLeft(), 0)

        let escEngine = RustEngine()
        _ = typeTelex(escEngine, "boo")
        _ = escEngine.processEscape()
        XCTAssertEqual(escEngine.lastWordForDict(), "boo")

        XCTAssertTrue(engine.applyProfile("Lập trình"))
        let profJSON = engine.getConfigJSON()
        let profDecoded = try JSONDecoder().decode(MSKeyConfig.self, from: XCTUnwrap(profJSON.data(using: .utf8)))
        XCTAssertEqual(profDecoded.activeProfile, "Lập trình")
        XCTAssertTrue(profDecoded.macroTriggerTabOnly)
        XCTAssertFalse(engine.applyProfile("NonExistentProfile"))

        XCTAssertTrue(RustEngine.isNewerVersion(current: "0.1.2", candidate: "v0.1.3"))
        XCTAssertFalse(RustEngine.isNewerVersion(current: "0.2.0", candidate: "0.2.0"))
        XCTAssertFalse(RustEngine.isNewerVersion(current: "0.2.0", candidate: "0.1.9"))
    }

    func testUpdateCheckerJSONAndSHA256Verification() throws {
        let releaseJSON = """
        {
          "draft": false,
          "tag_name": "v0.2.0",
          "html_url": "https://github.com/buidangminh23/ms-key/releases/tag/v0.2.0",
          "body": "Release notes",
          "assets": [
            {
              "name": "MSKey-0.2.0-macOS.dmg",
              "browser_download_url": "https://example.com/MSKey-0.2.0-macOS.dmg",
              "size": 1048576
            },
            {
              "name": "SHA256SUMS",
              "browser_download_url": "https://example.com/SHA256SUMS",
              "size": 256
            }
          ]
        }
        """
        let info = try XCTUnwrap(UpdateChecker.parseReleaseJSON(releaseJSON))
        XCTAssertEqual(info.version, "0.2.0")
        XCTAssertEqual(UpdateChecker.preferredMacAsset(from: info.assets)?.name, "MSKey-0.2.0-macOS.dmg")
        XCTAssertEqual(UpdateChecker.checksumAsset(from: info.assets)?.name, "SHA256SUMS")

        let payload = Data("mskey-release-binary".utf8)
        let hex = UpdateChecker.sha256Hex(of: payload)
        let sums = UpdateChecker.parseSHA256Sums("\(hex)  *./MSKey-0.2.0-macOS.dmg\n")
        XCTAssertTrue(UpdateChecker.verifySHA256(sums: sums, filename: "MSKey-0.2.0-macOS.dmg", actualHex: hex))
        XCTAssertFalse(UpdateChecker.verifySHA256(sums: sums, filename: "MSKey-0.2.0-macOS.dmg", actualHex: String(repeating: "0", count: 64)))

        let mixedAssets = [
            GitHubReleaseAsset(name: "mskey-cli-0.2.0-macos-universal.tar.gz", downloadURL: "https://example.com/cli.tar.gz"),
            GitHubReleaseAsset(name: "MSKey-0.2.0-macos-universal.dmg", downloadURL: "https://example.com/app.dmg"),
            GitHubReleaseAsset(name: "MSKey-0.2.0-macos-universal.zip", downloadURL: "https://example.com/app.zip"),
        ]
        XCTAssertEqual(
            UpdateChecker.preferredMacAsset(from: mixedAssets, preferZipForSelfUpdate: true)?.name,
            "MSKey-0.2.0-macos-universal.zip"
        )
        XCTAssertEqual(
            UpdateChecker.preferredMacAsset(from: mixedAssets, preferZipForSelfUpdate: false)?.name,
            "MSKey-0.2.0-macos-universal.dmg"
        )
        let replaceScript = UpdateChecker.buildSelfReplaceShellScript(
            pid: 999999,
            stagedAppPath: "/tmp/work/MSKey.app",
            targetAppPath: "/Applications/MSKey.app",
            workDirPath: "/tmp/work"
        )
        XCTAssertTrue(replaceScript.contains("xattr -dr com.apple.quarantine"))
        XCTAssertTrue(replaceScript.contains("Contents/Resources/macos-local-sign.sh"))
        XCTAssertTrue(replaceScript.contains("open -n '/Applications/MSKey.app'"))
    }

    func testTapHealthRestartsAfterPermissionReturnsAndStopsWhenRevoked() {
        XCTAssertEqual(EventTapManager.healthAction(trusted: true, state: .stopped), .start)
        XCTAssertEqual(EventTapManager.healthAction(trusted: true, state: .disabled), .recover)
        XCTAssertEqual(EventTapManager.healthAction(trusted: true, state: .starting), .none)
        XCTAssertEqual(EventTapManager.healthAction(trusted: true, state: .enabled), .none)
        XCTAssertEqual(EventTapManager.healthAction(trusted: false, state: .enabled), .stop)
        XCTAssertEqual(EventTapManager.healthAction(trusted: false, state: .disabled), .stop)
        XCTAssertEqual(EventTapManager.healthAction(trusted: false, state: .starting), .stop)
        XCTAssertEqual(EventTapManager.healthAction(trusted: false, state: .stopped), .none)
    }

    @MainActor
    func testMenuBarLetterKeepsSizeWhenTypingIsUnavailable() {
        let ready = MenuBarController.letterImage(letter: "V", vietnamese: true)
        let blocked = MenuBarController.letterImage(letter: "V", vietnamese: true, ready: false)
        XCTAssertEqual(ready.size, blocked.size)
        XCTAssertGreaterThanOrEqual(blocked.size.width, 18)
    }
}
