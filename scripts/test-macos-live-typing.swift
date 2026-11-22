import AppKit
import CoreGraphics
import Foundation

struct Stroke {
    let time: Int
    let key: CGKeyCode
    let flags: CGEventFlags
    let down: Bool
    let ascii: UInt16?
}

func typingCases(english: Bool) -> [(String, String)] {
    let alphabet = "abcdefghijklmnopqrstuvwxyz"
    let punctuation = ##"!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"##
    let alternating = zip(alphabet, alphabet.uppercased()).map { "\($0)\($1)" }.joined()
    var cases = [alphabet, alphabet.uppercased(), alternating, "0123456789", punctuation,
                 "https://example.com/test?id=42 qa@example.com"].map { ($0, $0) }
    if english {
        for letter in alphabet + alphabet.uppercased() {
            let triple = String(repeating: String(letter), count: 3)
            cases.append((String(letter), String(letter)))
            cases.append((triple, triple))
        }
        cases.append(("cacs hoawcj ddaayf", "cacs hoawcj ddaayf"))
        return cases
    }

    let modifiers: [Character: String] = ["ă": "aw", "â": "aa", "đ": "dd", "ê": "ee", "ô": "oo", "ơ": "ow", "ư": "uw"]
    for letter in "aăâbcdđeêghiklmnoôơpqrstuưvxy" {
        let keys = modifiers[letter] ?? String(letter)
        cases.append((keys, String(letter)))
        cases.append((keys.uppercased(), letter.uppercased()))
    }
    let vowels = ["aàáảãạ", "ăằắẳẵặ", "âầấẩẫậ", "eèéẻẽẹ", "êềếểễệ", "iìíỉĩị",
                  "oòóỏõọ", "ôồốổỗộ", "ơờớởỡợ", "uùúủũụ", "ưừứửữự", "yỳýỷỹỵ"]
    for row in vowels {
        let keys = modifiers[row.first!] ?? String(row.first!)
        for (letter, tone) in zip(row, ["", "f", "s", "r", "x", "j"]) {
            cases.append((keys + tone, String(letter)))
            cases.append(((keys + tone).uppercased(), letter.uppercased()))
        }
    }
    cases += [
        ("cacs cacs cacs; casc hoawjc ddaafy!", "các các các; các hoặc đầy!"),
        ("hoac hoawcj hoacwj ddaayf CACS HOAWCJ DDAAYF", "hoac hoặc hoặc đầy CÁC HOẶC ĐẦY"),
        ("ddang ddawng ddeer ddawtj", "đang đăng để đặt"),
        ("nguwowif chuwx tieengs vieejt", "người chữ tiếng việt"),
        ("truwowngf khoong quyeenf nuoots", "trường không quyền nuốt"),
        ("aa aaa dd ddd ee eee oo ooo ww www", "â aa đ dd ê ee ô oo w ww"),
        ("huyeets aps, tim machj, khowps, vieem, xoang.", "huyết áp, tim mạch, khớp, viêm, xoang."),
        ("API v2.4: cacs file JSON cofn ddaayf ddur.", "API v2.4: các file JSON còn đầy đủ."),
        ("CPU 95%, RAM 32GB; cacs tuwf Telex vieets ddungs.", "CPU 95%, RAM 32GB; các từ Telex viết đúng."),
    ]
    return cases
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard arguments.isEmpty || arguments == ["--english"] else {
    print("Usage: swift scripts/test-macos-live-typing.swift [--english]")
    exit(2)
}
let cases = typingCases(english: arguments == ["--english"])
let keys = cases.map { $0.0 }.joined(separator: " ") + " "
let expected = String(repeating: cases.map { $0.1 }.joined(separator: " ") + " ", count: 2)
let plain = "abcdefghijklmnopqrstuvwxyz0123456789`-=[]\\;',./ "
let shifted = "ABCDEFGHIJKLMNOPQRSTUVWXYZ)!@#$%^&*(~_+{}|:\"<>?"
let codes: [CGKeyCode] = [
    0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16, 6,
    29, 18, 19, 20, 21, 23, 22, 26, 28, 25, 50, 27, 24, 33, 30, 42, 41, 39, 43, 47, 44, 49,
]
let plainCodes = zip(plain, codes).map { ($0, ($1, CGEventFlags())) }
let shiftedCodes = zip(shifted, codes).map { ($0, ($1, CGEventFlags.maskShift)) }
let keyCodes = Dictionary(uniqueKeysWithValues: plainCodes + shiftedCodes)

let rawControl = plain + shifted + " aAbBcCdD 123!@# "

func makeSchedule(_ text: String) -> [Stroke] {
    var strokes: [Stroke] = []
    var releases: [CGKeyCode: Int] = [:]
    var lastRelease = -2
    var heldFlags = CGEventFlags()
    var tick = 2
    for hold in [2, 22] {
        for letter in text {
            let (key, flags) = keyCodes[letter]!
            tick = max(tick, (releases[key] ?? -1) + 1)
            if flags != heldFlags {
                tick = max(tick, lastRelease + 2)
                strokes.append(Stroke(time: tick - 1, key: 56, flags: flags, down: flags.contains(.maskShift), ascii: nil))
                heldFlags = flags
            }
            let ascii = UInt16(letter.asciiValue!)
            strokes.append(Stroke(time: tick, key: key, flags: flags, down: true, ascii: ascii))
            strokes.append(Stroke(time: tick + hold, key: key, flags: flags, down: false, ascii: ascii))
            releases[key] = tick + hold
            lastRelease = max(lastRelease, tick + hold)
            tick += 9
        }
    }
    if heldFlags.contains(.maskShift) {
        strokes.append(Stroke(time: lastRelease + 1, key: 56, flags: [], down: false, ascii: nil))
    }
    return strokes.sorted { $0.time < $1.time }
}

func hasMSKeyTap() -> Bool {
    let pids = Set(NSRunningApplication.runningApplications(withBundleIdentifier: "org.mskey.mac")
        .map(\.processIdentifier))
    var count: UInt32 = 0
    guard CGGetEventTapList(0, nil, &count) == .success, count > 0 else { return false }
    var taps = [CGEventTapInformation](repeating: .init(), count: Int(count))
    guard CGGetEventTapList(count, &taps, &count) == .success else { return false }
    return taps.prefix(Int(count)).contains {
        pids.contains($0.tappingProcess) && $0.enabled && $0.eventsOfInterest & (1 << CGEventType.keyDown.rawValue) != 0
    }
}

let app = NSApplication.shared
guard CGPreflightPostEventAccess(), hasMSKeyTap() else {
    print("FAIL prerequisites")
    exit(2)
}
let previous = NSWorkspace.shared.frontmostApplication
let window = NSWindow(contentRect: NSRect(x: 160, y: 200, width: 550, height: 180),
                      styleMask: [.titled], backing: .buffered, defer: false)
window.title = "MS Key fast typing regression"
let editor = NSTextView(frame: window.contentView!.bounds)
editor.isAutomaticSpellingCorrectionEnabled = false
editor.isAutomaticTextCompletionEnabled = false
editor.isAutomaticTextReplacementEnabled = false
editor.isAutomaticQuoteSubstitutionEnabled = false
editor.isAutomaticDashSubstitutionEnabled = false
editor.smartInsertDeleteEnabled = false
window.contentView = editor
app.setActivationPolicy(.regular)
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(editor)
app.activate(ignoringOtherApps: true)

let pid = ProcessInfo.processInfo.processIdentifier

@MainActor
enum TypingTest {
    static var focusObserver: NSObjectProtocol?

    static func checkOutput(control: Bool, deadline: TimeInterval) {
        guard app.isActive, window.isKeyWindow, window.firstResponder === editor else {
            print("FAIL focus")
            exit(3)
        }
        let output = editor.string
        let wanted = control ? rawControl + rawControl : expected
        let matched = output == wanted
        if !matched && ProcessInfo.processInfo.systemUptime < deadline {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { checkOutput(control: control, deadline: deadline) }
            return
        }
        print("\(matched ? "PASS" : "FAIL") \(control ? "raw-control" : "typing") keys=\((control ? rawControl : keys).count * 2) characters=\(output.count)/\(wanted.count)")
        if control && matched {
            editor.string = ""
            inject(control: false)
            return
        }
        if let focusObserver { NotificationCenter.default.removeObserver(focusObserver) }
        previous?.activate()
        exit(matched ? 0 : 1)
    }

    static func startWhenFocused(deadline: TimeInterval) {
        guard app.isActive, window.isKeyWindow, window.firstResponder === editor else {
            if ProcessInfo.processInfo.systemUptime < deadline {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { startWhenFocused(deadline: deadline) }
                return
            }
            print("FAIL focus")
            exit(3)
        }
        focusObserver = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: window, queue: .main) { _ in
            print("FAIL focus")
            exit(3)
        }
        inject(control: true)
    }

    static func inject(control: Bool) {
        let schedule = makeSchedule(control ? rawControl : keys)
        let stallIndices = control ? [] : Set([1, 3, 5].map { schedule.count * $0 / 8 })
        DispatchQueue.global(qos: .userInteractive).async {
            let source = CGEventSource(stateID: .privateState)
            source?.localEventsSuppressionInterval = 0
            let start = ProcessInfo.processInfo.systemUptime
            for (index, stroke) in schedule.enumerated() {
                let delay = start + Double(stroke.time) / 1000 - ProcessInfo.processInfo.systemUptime
                if delay > 0 { Thread.sleep(forTimeInterval: delay) }
                guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
                    print("FAIL focus")
                    exit(3)
                }
                guard let event = CGEvent(keyboardEventSource: source, virtualKey: stroke.key, keyDown: stroke.down) else {
                    print("FAIL event")
                    exit(4)
                }
                event.flags = stroke.flags
                if var ascii = stroke.ascii {
                    event.keyboardSetUnicodeString(stringLength: 1, unicodeString: &ascii)
                } else {
                    event.type = .flagsChanged
                }
                if control {
                    event.postToPid(pid)
                } else {
                    event.post(tap: .cghidEventTap)
                }
                if stallIndices.contains(index) {
                    DispatchQueue.main.async { Thread.sleep(forTimeInterval: 2.5) }
                }
            }
            DispatchQueue.main.async {
                let deadline = ProcessInfo.processInfo.systemUptime + 15
                DispatchQueue.main.asyncAfter(deadline: .now() + (control ? 1 : 4)) { checkOutput(control: control, deadline: deadline) }
            }
        }
    }
}

DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
    TypingTest.startWhenFocused(deadline: ProcessInfo.processInfo.systemUptime + 5)
}
app.run()
