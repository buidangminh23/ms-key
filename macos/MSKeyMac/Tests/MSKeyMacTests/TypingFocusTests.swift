import ApplicationServices
import XCTest
@testable import MSKeyKit

final class TypingFocusTests: XCTestCase {
    private func focus(pid: pid_t = 21, title: String = "", secure: Bool = false) -> MacTypingFocus {
        MacTypingFocus(pid: pid, appName: "Editor", windowTitle: title, secureField: secure)
    }

    func testOnlyConfirmedNonsecureForegroundReceivesTyping() {
        let ordinary = focus()
        XCTAssertTrue(ordinary.allowsTyping(targetPID: 0, frontPID: 21, secureInput: false))
        XCTAssertTrue(ordinary.allowsTyping(targetPID: 21, frontPID: 21, secureInput: false))
        XCTAssertFalse(ordinary.allowsTyping(targetPID: 30, frontPID: 21, secureInput: false))
        XCTAssertFalse(ordinary.allowsTyping(targetPID: 21, frontPID: 30, secureInput: false))
        XCTAssertFalse(ordinary.allowsTyping(targetPID: 0, frontPID: nil, secureInput: false))
        XCTAssertFalse(ordinary.allowsTyping(targetPID: 21, frontPID: 21, secureInput: true))
        XCTAssertFalse(focus(secure: true).allowsTyping(targetPID: 21, frontPID: 21, secureInput: false))
    }

    func testSecureSubroleLookupFailsClosedOnUnresponsiveField() {
        XCTAssertEqual(MacAppDetector.secureFieldStatus(subrole: kAXSecureTextFieldSubrole, status: .success), true)
        XCTAssertEqual(MacAppDetector.secureFieldStatus(subrole: "AXSearchField", status: .success), false)
        XCTAssertEqual(MacAppDetector.secureFieldStatus(subrole: nil, status: .attributeUnsupported), false)
        XCTAssertNil(MacAppDetector.secureFieldStatus(subrole: nil, status: .cannotComplete))
        XCTAssertNil(MacAppDetector.secureFieldStatus(subrole: nil, status: .success))
    }

    func testResponsiveOpaqueWindowUsesSecureInputGateButTimeoutPassesKeysThrough() {
        for status: AXError in [.noValue, .attributeUnsupported, .notImplemented] {
            XCTAssertEqual(MacTypingFocusMonitor.securityWithoutField(status, hasWindow: true), false)
            XCTAssertNil(MacTypingFocusMonitor.securityWithoutField(status, hasWindow: false))
        }
        XCTAssertNil(MacTypingFocusMonitor.securityWithoutField(.cannotComplete, hasWindow: true))
        XCTAssertNil(MacTypingFocusMonitor.securityWithoutField(.invalidUIElement, hasWindow: true))
    }

    func testNotificationRegistrationRetriesAfterSlowAppStartup() {
        XCTAssertEqual(MacFocusSubscription(.cannotComplete), .pending)
        XCTAssertEqual(MacFocusSubscription(.success), .active)
        XCTAssertEqual(MacFocusSubscription(.notificationAlreadyRegistered), .active)
        XCTAssertEqual(MacFocusSubscription(.notificationUnsupported), .unsupported)
    }

    func testUnchangedFieldPreservesWordAndChangingFieldClearsIt() {
        let engine = RustEngine()
        let tap = EventTapManager(rust: engine, config: MSKeyConfig())
        let context = focus()
        tap.updateTypingFocus(context, targetChanged: true)
        for key in "cac".unicodeScalars { _ = engine.processKey(scalar: key.value) }
        tap.updateTypingFocus(context, targetChanged: false)
        XCTAssertEqual(engine.processKey(scalar: 115), KeyActionResult(replace: true, backspaces: 2, text: "ác"))

        tap.updateTypingFocus(context, targetChanged: true)
        for key in "cac".unicodeScalars { _ = engine.processKey(scalar: key.value) }
        tap.updateTypingFocus(context, targetChanged: true)
        XCTAssertEqual(engine.processKey(scalar: 115), .passThrough)
    }

    func testTitleNotificationReappliesRulesInsideSameApplication() {
        var config = MSKeyConfig()
        config.app_rules = [AppRuleItem(process_name: "Editor", titlePattern: "*terminal*", behavior: .forceEnglish)]
        let engine = RustEngine()
        let tap = EventTapManager(rust: engine, config: config)
        tap.updateConfig(config)
        tap.updateTypingFocus(focus(title: "Document"), targetChanged: true)
        XCTAssertTrue(engine.isEnabled())
        tap.updateTypingFocus(focus(title: "Terminal"), targetChanged: false)
        XCTAssertFalse(engine.isEnabled())
        tap.updateTypingFocus(focus(title: "Document"), targetChanged: false)
        XCTAssertTrue(engine.isEnabled())
    }
}
