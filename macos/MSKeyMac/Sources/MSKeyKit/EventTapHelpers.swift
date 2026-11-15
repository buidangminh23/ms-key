import AppKit
import Carbon
import Foundation

public enum MacKeycode {
    private static let vkMap: [Int: UInt32] = [
        kVK_ANSI_A: 0x41, kVK_ANSI_B: 0x42, kVK_ANSI_C: 0x43, kVK_ANSI_D: 0x44,
        kVK_ANSI_E: 0x45, kVK_ANSI_F: 0x46, kVK_ANSI_G: 0x47, kVK_ANSI_H: 0x48,
        kVK_ANSI_I: 0x49, kVK_ANSI_J: 0x4A, kVK_ANSI_K: 0x4B, kVK_ANSI_L: 0x4C,
        kVK_ANSI_M: 0x4D, kVK_ANSI_N: 0x4E, kVK_ANSI_O: 0x4F, kVK_ANSI_P: 0x50,
        kVK_ANSI_Q: 0x51, kVK_ANSI_R: 0x52, kVK_ANSI_S: 0x53, kVK_ANSI_T: 0x54,
        kVK_ANSI_U: 0x55, kVK_ANSI_V: 0x56, kVK_ANSI_W: 0x57, kVK_ANSI_X: 0x58,
        kVK_ANSI_Y: 0x59, kVK_ANSI_Z: 0x5A,
        kVK_ANSI_0: 0x30, kVK_ANSI_1: 0x31, kVK_ANSI_2: 0x32, kVK_ANSI_3: 0x33,
        kVK_ANSI_4: 0x34, kVK_ANSI_5: 0x35, kVK_ANSI_6: 0x36, kVK_ANSI_7: 0x37,
        kVK_ANSI_8: 0x38, kVK_ANSI_9: 0x39,
        kVK_F1: 0x70, kVK_F2: 0x71, kVK_F3: 0x72, kVK_F4: 0x73,
        kVK_F5: 0x74, kVK_F6: 0x75, kVK_F7: 0x76, kVK_F8: 0x77,
        kVK_F9: 0x78, kVK_F10: 0x79, kVK_F11: 0x7A, kVK_F12: 0x7B,
        kVK_Space: 0x20, kVK_Return: 0x0D, kVK_ANSI_KeypadEnter: 0x0D, kVK_Tab: 0x09,
        kVK_Delete: 0x08, kVK_ForwardDelete: 0x2E, kVK_Escape: 0x1B,
        kVK_ANSI_Semicolon: 0xBA, kVK_ANSI_Equal: 0xBB, kVK_ANSI_Comma: 0xBC,
        kVK_ANSI_Minus: 0xBD, kVK_ANSI_Period: 0xBE, kVK_ANSI_Slash: 0xBF,
        kVK_ANSI_Grave: 0xC0, kVK_ANSI_LeftBracket: 0xDB, kVK_ANSI_Backslash: 0xDC,
        kVK_ANSI_RightBracket: 0xDD, kVK_ANSI_Quote: 0xDE,
    ]

    public static func toPortableVK(_ keyCode: UInt16) -> UInt32 {
        vkMap[Int(keyCode)] ?? UInt32(keyCode)
    }

    public static func isNavigationKey(_ keyCode: UInt16) -> Bool {
        switch Int(keyCode) {
        case kVK_LeftArrow, kVK_RightArrow, kVK_DownArrow, kVK_UpArrow,
             kVK_Home, kVK_End, kVK_PageUp, kVK_PageDown, kVK_ForwardDelete, kVK_Help:
            return true
        default:
            return false
        }
    }

    public static func selectionDigit(for keyCode: UInt16) -> Int? {
        switch Int(keyCode) {
        case kVK_ANSI_1, kVK_ANSI_Keypad1: return 1
        case kVK_ANSI_2, kVK_ANSI_Keypad2: return 2
        case kVK_ANSI_3, kVK_ANSI_Keypad3: return 3
        case kVK_ANSI_4, kVK_ANSI_Keypad4: return 4
        case kVK_ANSI_5, kVK_ANSI_Keypad5: return 5
        case kVK_ANSI_6, kVK_ANSI_Keypad6: return 6
        case kVK_ANSI_7, kVK_ANSI_Keypad7: return 7
        case kVK_ANSI_8, kVK_ANSI_Keypad8: return 8
        default: return nil
        }
    }
}

struct SendOptions {
    let useClipboard: Bool
    let pasteShiftInsert: Bool
    let standardMode: Bool
    let delayMs: Int
    let browserFix1: Bool
    let browserFix2: Bool
}

let mskeyEventTag: Int64 = 0x4D53_4B59

enum CGEventEmitter {
    static func postVirtualKey(_ vk: CGKeyCode, flags: CGEventFlags, delayMs: Int) {
        let src = CGEventSource(stateID: .privateState)
        guard let down = CGEvent(keyboardEventSource: src, virtualKey: vk, keyDown: true),
              let up = CGEvent(keyboardEventSource: src, virtualKey: vk, keyDown: false) else {
            return
        }
        down.flags = flags
        up.flags = flags
        down.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
        up.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        if delayMs > 0 {
            usleep(UInt32(delayMs) * 1000)
        }
    }

    static func postBackspaces(_ count: Int, delayMs: Int) {
        for _ in 0..<count {
            postVirtualKey(CGKeyCode(kVK_Delete), flags: [], delayMs: delayMs)
        }
    }

    static func postUnicodeText(_ text: String, singleChar: Bool, delayMs: Int) {
        let src = CGEventSource(stateID: .privateState)
        if singleChar {
            for scalar in text.unicodeScalars {
                var units = Array(String(scalar).utf16)
                postUnicodeUnits(&units, src: src, delayMs: delayMs)
            }
            return
        }
        let units = Array(text.utf16)
        var offset = 0
        while offset < units.count {
            let end = min(offset + 16, units.count)
            var slice = Array(units[offset..<end])
            postUnicodeUnits(&slice, src: src, delayMs: delayMs)
            offset = end
        }
    }

    private static func postUnicodeUnits(_ units: inout [UniChar], src: CGEventSource?, delayMs: Int) {
        guard let down = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: true),
              let up = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: false) else {
            return
        }
        down.flags = []
        up.flags = []
        down.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
        up.setIntegerValueField(.eventSourceUserData, value: mskeyEventTag)
        down.keyboardSetUnicodeString(stringLength: units.count, unicodeString: &units)
        up.keyboardSetUnicodeString(stringLength: units.count, unicodeString: &units)
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        if delayMs > 0 {
            usleep(UInt32(delayMs) * 1000)
        }
    }
}

enum PasteboardSnapshot {
    static func capture(_ pb: NSPasteboard) -> [[(String, Data)]] {
        var result: [[(String, Data)]] = []
        for item in pb.pasteboardItems ?? [] {
            var entries: [(String, Data)] = []
            for type in item.types {
                if let data = item.data(forType: type) {
                    entries.append((type.rawValue, data))
                }
            }
            if !entries.isEmpty {
                result.append(entries)
            }
        }
        return result
    }

    static func restore(_ saved: [[(String, Data)]], to pb: NSPasteboard) {
        pb.clearContents()
        guard !saved.isEmpty else { return }
        var items: [NSPasteboardItem] = []
        for entries in saved {
            let item = NSPasteboardItem()
            for (rawType, data) in entries {
                item.setData(data, forType: NSPasteboard.PasteboardType(rawType))
            }
            items.append(item)
        }
        pb.writeObjects(items)
    }
}
