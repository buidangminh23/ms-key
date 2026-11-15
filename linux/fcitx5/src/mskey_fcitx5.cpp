#include "mskey_fcitx5.h"

#include <sys/stat.h>
#include <time.h>

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/surroundingtext.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h>

namespace mskey {

namespace {

int64_t fileMtimeNanos(const std::string &path) {
    struct stat st {};
    if (path.empty() || stat(path.c_str(), &st) != 0) {
        return -1;
    }
    return static_cast<int64_t>(st.st_mtim.tv_sec) * 1000000000LL + static_cast<int64_t>(st.st_mtim.tv_nsec);
}

std::string trimCopy(const std::string &s) {
    size_t start = 0;
    while (start < s.size() && std::isspace(static_cast<unsigned char>(s[start]))) {
        start++;
    }
    size_t end = s.size();
    while (end > start && std::isspace(static_cast<unsigned char>(s[end - 1]))) {
        end--;
    }
    return s.substr(start, end - start);
}

std::string expandHomePath(const std::string &raw) {
    std::string trimmed = trimCopy(raw);
    if (trimmed.empty()) {
        return "";
    }
    if (trimmed == "~" || trimmed.rfind("~/", 0) == 0) {
        const char *home = std::getenv("HOME");
        std::string base = home ? home : ".";
        if (trimmed.size() == 1) {
            return base;
        }
        return base + trimmed.substr(1);
    }
    return trimmed;
}

std::string configDirPath() {
    const char *xdg = std::getenv("XDG_CONFIG_HOME");
    if (xdg && xdg[0] != '\0') {
        return std::string(xdg) + "/mskey";
    }
    const char *home = std::getenv("HOME");
    return std::string(home ? home : ".") + "/.config/mskey";
}

std::string primaryConfigPath() {
    return configDirPath() + "/config.json";
}

void saveLastEscWord(MsKeyHandle *handle) {
    char *raw = mskey_last_word_for_dict(handle);
    if (!raw) {
        return;
    }
    std::string word = trimCopy(raw);
    mskey_free_string(raw);
    if (word.empty()) {
        return;
    }
    std::string path = configDirPath() + "/last_esc_word.txt";
    std::ofstream ofs(path, std::ios::trunc);
    if (ofs.is_open()) {
        ofs << word;
    }
}

bool readFileString(const std::string &path, std::string &out) {
    if (path.empty()) {
        return false;
    }
    std::ifstream ifs(path);
    if (!ifs.is_open()) {
        return false;
    }
    std::ostringstream ss;
    ss << ifs.rdbuf();
    out = ss.str();
    return !out.empty();
}

int64_t localUtcOffsetSecs() {
    time_t now = time(nullptr);
    struct tm localTm {};
    if (localtime_r(&now, &localTm) != nullptr) {
        return static_cast<int64_t>(localTm.tm_gmtoff);
    }
    return 0;
}

size_t skipJsonWs(const std::string &s, size_t i) {
    while (i < s.size() && std::isspace(static_cast<unsigned char>(s[i]))) {
        i++;
    }
    return i;
}

bool parseJsonStringAt(const std::string &s, size_t &i, std::string &out) {
    if (i >= s.size() || s[i] != '"') {
        return false;
    }
    i++;
    out.clear();
    while (i < s.size()) {
        char c = s[i++];
        if (c == '"') {
            return true;
        }
        if (c == '\\' && i < s.size()) {
            char esc = s[i++];
            switch (esc) {
            case '"': out.push_back('"'); break;
            case '\\': out.push_back('\\'); break;
            case '/': out.push_back('/'); break;
            case 'n': out.push_back('\n'); break;
            case 'r': out.push_back('\r'); break;
            case 't': out.push_back('\t'); break;
            default: out.push_back(esc); break;
            }
        } else {
            out.push_back(c);
        }
    }
    return false;
}

size_t skipJsonValue(const std::string &s, size_t i) {
    i = skipJsonWs(s, i);
    if (i >= s.size()) {
        return i;
    }
    if (s[i] == '"') {
        std::string tmp;
        parseJsonStringAt(s, i, tmp);
        return i;
    }
    if (s[i] == '{' || s[i] == '[') {
        char openCh = s[i];
        char closeCh = openCh == '{' ? '}' : ']';
        int depth = 0;
        while (i < s.size()) {
            if (s[i] == '"') {
                std::string tmp;
                parseJsonStringAt(s, i, tmp);
                continue;
            }
            if (s[i] == openCh) {
                depth++;
            } else if (s[i] == closeCh) {
                depth--;
                if (depth == 0) {
                    return i + 1;
                }
            }
            i++;
        }
        return i;
    }
    while (i < s.size() && s[i] != ',' && s[i] != '}' && s[i] != ']'
           && !std::isspace(static_cast<unsigned char>(s[i]))) {
        i++;
    }
    return i;
}

bool findTopLevelFieldValue(const std::string &jsonObj, const char *targetKey, size_t &valStart, size_t &valEnd) {
    size_t i = skipJsonWs(jsonObj, 0);
    if (i >= jsonObj.size() || jsonObj[i] != '{') {
        return false;
    }
    i++;
    while (i < jsonObj.size()) {
        i = skipJsonWs(jsonObj, i);
        if (i >= jsonObj.size() || jsonObj[i] == '}') {
            return false;
        }
        if (jsonObj[i] == ',') {
            i++;
            continue;
        }
        std::string key;
        if (!parseJsonStringAt(jsonObj, i, key)) {
            return false;
        }
        i = skipJsonWs(jsonObj, i);
        if (i >= jsonObj.size() || jsonObj[i] != ':') {
            return false;
        }
        i = skipJsonWs(jsonObj, i + 1);
        size_t end = skipJsonValue(jsonObj, i);
        if (key == targetKey) {
            valStart = i;
            valEnd = end;
            return true;
        }
        i = end;
    }
    return false;
}

bool jsonBoolField(const std::string &obj, const char *key, bool fallback) {
    size_t a = 0;
    size_t b = 0;
    if (!findTopLevelFieldValue(obj, key, a, b)) {
        return fallback;
    }
    std::string val = obj.substr(a, b - a);
    if (val == "true") {
        return true;
    }
    if (val == "false") {
        return false;
    }
    return fallback;
}

std::string jsonStrField(const std::string &obj, const char *key, const std::string &fallback) {
    size_t a = 0;
    size_t b = 0;
    if (!findTopLevelFieldValue(obj, key, a, b)) {
        return fallback;
    }
    std::string out;
    size_t cursor = a;
    if (parseJsonStringAt(obj, cursor, out)) {
        return out;
    }
    return fallback;
}

uint32_t jsonU32Field(const std::string &obj, const char *key, uint32_t fallback) {
    size_t a = 0;
    size_t b = 0;
    if (!findTopLevelFieldValue(obj, key, a, b)) {
        return fallback;
    }
    std::string raw = obj.substr(a, b - a);
    char *endPtr = nullptr;
    unsigned long parsed = std::strtoul(raw.c_str(), &endPtr, 10);
    if (endPtr == raw.c_str()) {
        return fallback;
    }
    return static_cast<uint32_t>(parsed);
}

KeyCombo parseKeyComboField(const std::string &obj, const char *key) {
    KeyCombo out;
    size_t a = 0;
    size_t b = 0;
    if (!findTopLevelFieldValue(obj, key, a, b)) {
        return out;
    }
    std::string sub = obj.substr(a, b - a);
    out.ctrl = jsonBoolField(sub, "ctrl", false);
    out.alt = jsonBoolField(sub, "alt", false);
    out.shift = jsonBoolField(sub, "shift", false);
    out.winCmd = jsonBoolField(sub, "win_cmd", false);
    out.keyCode = jsonU32Field(sub, "key_code", 0);
    return out;
}

ParsedSettings parseSettingsFromJson(const std::string &json) {
    ParsedSettings s;
    s.vietnameseEnabled = jsonBoolField(json, "vietnamese_enabled", true);
    s.usePreedit = jsonStrField(json, "mechanism", "KeyboardHook") == "KeyboardHook";
    s.macroEnabled = jsonBoolField(json, "macro_enabled", true);
    s.macroWhenEnglish = jsonBoolField(json, "macro_when_english", false);
    s.macroTriggerTabOnly = jsonBoolField(json, "macro_trigger_tab_only", false);
    s.smartAppSwitch = jsonBoolField(json, "smart_app_switch", true);
    s.quickCapslockSwitch = jsonBoolField(json, "quick_capslock_switch", false);
    s.reeditSurroundingWord = jsonBoolField(json, "reedit_surrounding_word", true);
    s.syncConfigPath = jsonStrField(json, "sync_config_path", "");

    std::string holdStr = jsonStrField(json, "hold_english_key", "None");
    if (holdStr == "RightShift") {
        s.holdEnglishKey = HoldEnglishKeyMode::RightShift;
    } else if (holdStr == "LeftShift") {
        s.holdEnglishKey = HoldEnglishKeyMode::LeftShift;
    } else if (holdStr == "RightAlt") {
        s.holdEnglishKey = HoldEnglishKeyMode::RightAlt;
    } else if (holdStr == "RightCtrl") {
        s.holdEnglishKey = HoldEnglishKeyMode::RightCtrl;
    } else if (holdStr == "CapsLock") {
        s.holdEnglishKey = HoldEnglishKeyMode::CapsLock;
    } else {
        s.holdEnglishKey = HoldEnglishKeyMode::None;
    }

    std::string presetStr = jsonStrField(json, "switch_preset", "CtrlShift");
    if (presetStr == "AltZ") {
        s.switchPreset = SwitchPresetMode::AltZ;
    } else if (presetStr == "CtrlSpace") {
        s.switchPreset = SwitchPresetMode::CtrlSpace;
    } else if (presetStr == "ShiftShift") {
        s.switchPreset = SwitchPresetMode::ShiftShift;
    } else if (presetStr == "Custom") {
        s.switchPreset = SwitchPresetMode::Custom;
    } else {
        s.switchPreset = SwitchPresetMode::CtrlShift;
    }

    s.customSwitchHotkey = parseKeyComboField(json, "custom_switch_hotkey");
    s.undoLastWordHotkey = parseKeyComboField(json, "undo_last_word_hotkey");

    size_t mA = 0;
    size_t mB = 0;
    if (findTopLevelFieldValue(json, "macros", mA, mB)) {
        std::string macrosArr = json.substr(mA, mB - mA);
        s.hasClipboardMacro = macrosArr.find("{clipboard}") != std::string::npos;
    }
    return s;
}

void appendUtf8(std::string &out, uint32_t cp) {
    if (cp <= 0x7F) {
        out.push_back(static_cast<char>(cp));
    } else if (cp <= 0x7FF) {
        out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else if (cp <= 0xFFFF) {
        out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else if (cp <= 0x10FFFF) {
        out.push_back(static_cast<char>(0xF0 | (cp >> 18)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    }
}

void eraseLastCodepoints(std::string &s, uint32_t count) {
    while (count > 0 && !s.empty()) {
        while (!s.empty() && (static_cast<unsigned char>(s.back()) & 0xC0) == 0x80) {
            s.pop_back();
        }
        if (!s.empty()) {
            s.pop_back();
        }
        count--;
    }
}

void applyReplaceToUtf8(std::string &buf, uint32_t backspaces, const char *text) {
    eraseLastCodepoints(buf, backspaces);
    if (text && text[0] != '\0') {
        buf.append(text);
    }
}

uint32_t utf8CodepointLength(const std::string &s) {
    uint32_t count = 0;
    for (unsigned char b : s) {
        if ((b & 0xC0) != 0x80) {
            count++;
        }
    }
    return count;
}

std::string utf8PrefixByCodepoints(const std::string &s, unsigned int chars) {
    unsigned int count = 0;
    size_t i = 0;
    while (i < s.size() && count < chars) {
        unsigned char b = static_cast<unsigned char>(s[i]);
        size_t step = 1;
        if ((b & 0x80) == 0) {
            step = 1;
        } else if ((b & 0xE0) == 0xC0) {
            step = 2;
        } else if ((b & 0xF0) == 0xE0) {
            step = 3;
        } else if ((b & 0xF8) == 0xF0) {
            step = 4;
        }
        if (i + step > s.size()) {
            break;
        }
        i += step;
        count++;
    }
    return s.substr(0, i);
}

bool isDelimiterChar(uint32_t cp) {
    if (cp == ' ' || cp == '\t' || cp == '\n' || cp == '\r') {
        return true;
    }
    switch (cp) {
    case ',': case '.': case ';': case ':': case '!': case '?':
    case '(': case ')': case '{': case '}': case '[': case ']':
    case '/': case '\\': case '-': case '+': case '=': case '*':
    case '@': case '#': case '$': case '%': case '^': case '&':
    case '_': case '~': case '<': case '>': case '|': case '"':
    case '\'': case '`':
    case 0x2013: case 0x2014: case 0x2018: case 0x2019:
    case 0x201C: case 0x201D: case 0x2026:
        return true;
    default:
        return false;
    }
}

std::string extractTrailingWordUtf8(const std::string &beforeCursor) {
    if (beforeCursor.empty()) {
        return "";
    }
    size_t splitAt = beforeCursor.size();
    while (splitAt > 0) {
        size_t prev = splitAt - 1;
        while (prev > 0 && (static_cast<unsigned char>(beforeCursor[prev]) & 0xC0) == 0x80) {
            prev--;
        }
        unsigned char b0 = static_cast<unsigned char>(beforeCursor[prev]);
        uint32_t cp = b0;
        if ((b0 & 0xE0) == 0xC0 && prev + 1 < splitAt) {
            cp = ((b0 & 0x1F) << 6) | (static_cast<unsigned char>(beforeCursor[prev + 1]) & 0x3F);
        } else if ((b0 & 0xF0) == 0xE0 && prev + 2 < splitAt) {
            cp = ((b0 & 0x0F) << 12)
                 | ((static_cast<unsigned char>(beforeCursor[prev + 1]) & 0x3F) << 6)
                 | (static_cast<unsigned char>(beforeCursor[prev + 2]) & 0x3F);
        } else if ((b0 & 0xF8) == 0xF0 && prev + 3 < splitAt) {
            cp = ((b0 & 0x07) << 18)
                 | ((static_cast<unsigned char>(beforeCursor[prev + 1]) & 0x3F) << 12)
                 | ((static_cast<unsigned char>(beforeCursor[prev + 2]) & 0x3F) << 6)
                 | (static_cast<unsigned char>(beforeCursor[prev + 3]) & 0x3F);
        }
        if (isDelimiterChar(cp)) {
            break;
        }
        splitAt = prev;
    }
    return beforeCursor.substr(splitAt);
}

uint32_t keySymToCodepoint(fcitx::KeySym sym) {
    if (sym == FcitxKey_KP_Space) {
        return ' ';
    }
    if (sym >= FcitxKey_KP_0 && sym <= FcitxKey_KP_9) {
        return '0' + (sym - FcitxKey_KP_0);
    }
    switch (sym) {
    case FcitxKey_KP_Add: return '+';
    case FcitxKey_KP_Subtract: return '-';
    case FcitxKey_KP_Multiply: return '*';
    case FcitxKey_KP_Divide: return '/';
    case FcitxKey_KP_Decimal: return '.';
    case FcitxKey_KP_Equal: return '=';
    default: break;
    }
    uint32_t cp = fcitx::Key::keySymToUnicode(sym);
    if (cp >= 0x20 && cp != 0x7F) {
        return cp;
    }
    return 0;
}

bool isNavigationKeySym(fcitx::KeySym sym) {
    if (sym >= FcitxKey_Home && sym <= FcitxKey_Begin) {
        return true;
    }
    return sym == FcitxKey_Delete;
}

bool matchesHoldEnglishSym(HoldEnglishKeyMode mode, fcitx::KeySym sym) {
    switch (mode) {
    case HoldEnglishKeyMode::RightShift: return sym == FcitxKey_Shift_R;
    case HoldEnglishKeyMode::LeftShift: return sym == FcitxKey_Shift_L;
    case HoldEnglishKeyMode::RightAlt: return sym == FcitxKey_Alt_R;
    case HoldEnglishKeyMode::RightCtrl: return sym == FcitxKey_Control_R;
    case HoldEnglishKeyMode::CapsLock: return sym == FcitxKey_Caps_Lock;
    case HoldEnglishKeyMode::None: default: return false;
    }
}

bool matchesKeyCombo(const KeyCombo &combo, fcitx::KeySym sym, fcitx::KeyStates states) {
    if (combo.keyCode == 0) {
        return false;
    }
    if (states.test(fcitx::KeyState::Ctrl) != combo.ctrl
        || states.test(fcitx::KeyState::Alt) != combo.alt
        || states.test(fcitx::KeyState::Shift) != combo.shift
        || states.test(fcitx::KeyState::Super) != combo.winCmd) {
        return false;
    }
    uint32_t vk = combo.keyCode;
    if (vk >= 0x30 && vk <= 0x39) {
        return sym == vk;
    }
    if (vk >= 0x41 && vk <= 0x5A) {
        return sym == vk || sym == (vk + 32);
    }
    if (vk >= 0x70 && vk <= 0x7B) {
        return sym == static_cast<fcitx::KeySym>(FcitxKey_F1 + (vk - 0x70));
    }
    if (vk == 0x20) {
        return sym == FcitxKey_space || sym == FcitxKey_KP_Space;
    }
    if (vk == 0x0D) {
        return sym == FcitxKey_Return || sym == FcitxKey_KP_Enter;
    }
    if (vk == 0x09) {
        return sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab;
    }
    struct PunctPair {
        uint32_t vk;
        fcitx::KeySym unshifted;
        fcitx::KeySym shifted;
    };
    static const PunctPair kPuncts[] = {
        {0xBA, ';', ':'},
        {0xBB, '=', '+'},
        {0xBC, ',', '<'},
        {0xBD, '-', '_'},
        {0xBE, '.', '>'},
        {0xBF, '/', '?'},
        {0xC0, '`', '~'},
        {0xDB, '[', '{'},
        {0xDC, '\\', '|'},
        {0xDD, ']', '}'},
        {0xDE, '\'', '"'},
    };
    for (const auto &p : kPuncts) {
        if (p.vk == vk) {
            return sym == p.unshifted || sym == p.shifted;
        }
    }
    return sym == vk;
}

bool readCliOutput(const char *cmd, std::string &out) {
    FILE *pipe = popen(cmd, "r");
    if (!pipe) {
        return false;
    }
    out.clear();
    char buf[1024];
    while (size_t n = std::fread(buf, 1, sizeof(buf), pipe)) {
        out.append(buf, n);
        if (out.size() >= 65536) {
            break;
        }
    }
    int rc = pclose(pipe);
    return rc == 0 && !out.empty();
}

std::string readClipboardText() {
    std::string txt;
    const char *wayland = std::getenv("WAYLAND_DISPLAY");
    if (wayland && wayland[0] != '\0') {
        if (readCliOutput("wl-paste --no-newline 2>/dev/null", txt)
            || readCliOutput("wl-paste 2>/dev/null", txt)) {
            return txt;
        }
    }
    if (readCliOutput("xclip -o -selection clipboard 2>/dev/null", txt)
        || readCliOutput("xsel --clipboard --output 2>/dev/null", txt)) {
        return txt;
    }
    return "";
}

}

MsKeyState::MsKeyState() {
    char *defJson = mskey_default_config_json();
    handle = mskey_create(defJson ? defJson : "");
    if (defJson) {
        mskey_free_string(defJson);
    }
    if (handle) {
        mskey_set_clock_offset(handle, localUtcOffsetSecs());
        char *norm = mskey_get_config_json(handle);
        if (norm) {
            settings = parseSettingsFromJson(norm);
            preferredVietnamese = settings.vietnameseEnabled;
            mskey_free_string(norm);
        }
    }
}

MsKeyState::~MsKeyState() {
    if (handle) {
        mskey_destroy(handle);
        handle = nullptr;
    }
}

MsKeyEngine::MsKeyEngine(fcitx::Instance *instance)
    : instance_(instance),
      factory_([](fcitx::InputContext &) { return new MsKeyState(); }) {
    instance_->inputContextManager().registerProperty("mskeyState", &factory_);
    surroundingWatcher_ = instance_->watchEvent(
        fcitx::EventType::InputContextSurroundingTextUpdated,
        fcitx::EventWatcherPhase::Default,
        [this](fcitx::Event &event) {
            auto &icEvent = static_cast<fcitx::InputContextEvent &>(event);
            auto *ic = icEvent.inputContext();
            if (!ic) {
                return;
            }
            auto *state = ic->propertyFor(&factory_);
            if (!state || !state->active || !state->handle) {
                return;
            }
            if (state->suppressSurroundingCount > 0) {
                state->suppressSurroundingCount--;
                return;
            }
            if (!state->preedit.empty() || !state->settings.reeditSurroundingWord) {
                return;
            }
            hydrateFromSurrounding(ic, state);
        });
}

void MsKeyEngine::reloadConfigIfNeeded(MsKeyState *state) {
    if (!state || !state->handle) {
        return;
    }
    std::string primary = primaryConfigPath();
    int64_t primaryMtime = fileMtimeNanos(primary);
    std::string chosenJson;
    int64_t effectiveMtime = primaryMtime;

    std::string primaryRaw;
    if (readFileString(primary, primaryRaw)) {
        ParsedSettings tmp = parseSettingsFromJson(primaryRaw);
        std::string syncPath = expandHomePath(tmp.syncConfigPath);
        int64_t syncMtime = fileMtimeNanos(syncPath);
        if (!syncPath.empty() && syncPath != primary && syncMtime > primaryMtime) {
            std::string syncRaw;
            if (readFileString(syncPath, syncRaw)) {
                chosenJson = std::move(syncRaw);
                effectiveMtime = syncMtime;
            }
        }
        if (chosenJson.empty()) {
            chosenJson = std::move(primaryRaw);
        }
    }

    if (effectiveMtime == state->configMtimeNs) {
        return;
    }
    state->configMtimeNs = effectiveMtime;
    mskey_set_clock_offset(state->handle, localUtcOffsetSecs());
    if (!chosenJson.empty() && mskey_set_config_json(state->handle, chosenJson.c_str())) {
        char *norm = mskey_get_config_json(state->handle);
        if (norm) {
            state->settings = parseSettingsFromJson(norm);
            state->preferredVietnamese = state->settings.vietnameseEnabled;
            mskey_free_string(norm);
        }
    }
}

void MsKeyEngine::applyAppPolicy(fcitx::InputContext *ic, MsKeyState *state) {
    if (!ic || !state || !state->handle) {
        return;
    }
    const std::string &prog = ic->program();
    uint8_t behavior = prog.empty()
        ? 0
        : mskey_behavior_for_window(state->handle, prog.c_str(), "");
    state->appBlacklisted = (behavior == 3);
    bool target = state->preferredVietnamese;
    if (behavior == 1 || behavior == 3) {
        target = false;
    } else if (behavior == 2) {
        target = true;
    }
    mskey_set_enabled(state->handle, target);
}

void MsKeyEngine::hydrateFromSurrounding(fcitx::InputContext *ic, MsKeyState *state) {
    if (!ic || !state || !state->handle || !state->settings.reeditSurroundingWord) {
        return;
    }
    if (!ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText)
        || !ic->surroundingText().isValid()) {
        return;
    }
    const auto &sur = ic->surroundingText();
    std::string before = utf8PrefixByCodepoints(sur.text(), sur.cursor());
    mskey_hydrate_word(state->handle, before.c_str());
    state->hydratedWord = extractTrailingWordUtf8(before);
    state->hydratedLen = utf8CodepointLength(state->hydratedWord);
}

void MsKeyEngine::updateClientPreedit(fcitx::InputContext *ic, MsKeyState *state) {
    auto &panel = ic->inputPanel();
    panel.reset();
    if (!state->preedit.empty()) {
        fcitx::Text pre(state->preedit, fcitx::TextFormatFlag::Underline);
        pre.setCursor(static_cast<int>(state->preedit.size()));
        if (ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) {
            panel.setClientPreedit(pre);
        } else {
            panel.setPreedit(pre);
            ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        }
    } else {
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }
    ic->updatePreedit();
}

void MsKeyEngine::flushPreedit(fcitx::InputContext *ic, MsKeyState *state) {
    if (!state->preedit.empty()) {
        std::string text = std::move(state->preedit);
        state->preedit.clear();
        updateClientPreedit(ic, state);
        state->suppressSurroundingCount++;
        ic->commitString(text);
    }
}

void MsKeyEngine::commitDirectReplace(
    fcitx::InputContext *ic,
    MsKeyState *state,
    uint32_t backspaces,
    const std::string &text
) {
    if (backspaces > 0) {
        if (ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText)) {
            state->suppressSurroundingCount++;
            ic->deleteSurroundingText(-static_cast<int>(backspaces), backspaces);
        } else {
            for (uint32_t i = 0; i < backspaces; i++) {
                ic->forwardKey(fcitx::Key(FcitxKey_BackSpace));
            }
        }
    }
    if (!text.empty()) {
        state->suppressSurroundingCount++;
        ic->commitString(text);
    }
}

void MsKeyEngine::forwardPendingCursorLeft(fcitx::InputContext *ic, MsKeyHandle *handle) {
    uint32_t steps = mskey_take_pending_cursor_left(handle);
    for (uint32_t i = 0; i < steps; i++) {
        ic->forwardKey(fcitx::Key(FcitxKey_Left));
    }
}

void MsKeyEngine::resetHotkeyTracker(MsKeyState *state) {
    state->ctrlShiftPrimed = false;
    state->shiftTainted = false;
    state->shiftHeldForTap = false;
    state->hasLastShiftTap = false;
}

bool MsKeyEngine::handleUndoHotkey(fcitx::InputContext *ic, MsKeyState *state) {
    MsKeyAction act = mskey_toggle_last_committed_word(state->handle);
    if (act.kind != 1) {
        mskey_free_action(act);
        return false;
    }
    std::string text = act.text ? act.text : "";
    uint32_t bs = act.backspaces;
    mskey_free_action(act);
    if (!state->preedit.empty() && state->settings.usePreedit
        && ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) {
        applyReplaceToUtf8(state->preedit, bs, text.c_str());
        updateClientPreedit(ic, state);
    } else {
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        commitDirectReplace(ic, state, bs, text);
    }
    forwardPendingCursorLeft(ic, state->handle);
    return true;
}

void MsKeyEngine::activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (!ic) {
        return;
    }
    auto *state = ic->propertyFor(&factory_);
    if (!state || !state->handle) {
        return;
    }
    state->active = true;
    state->preedit.clear();
    state->hydratedWord.clear();
    state->hydratedLen = 0;
    state->suppressSurroundingCount = 0;
    state->holdEnglishActive = false;
    resetHotkeyTracker(state);
    mskey_set_temporary_english(state->handle, false);
    mskey_reset_all(state->handle);
    reloadConfigIfNeeded(state);
    applyAppPolicy(ic, state);
    updateClientPreedit(ic, state);
    hydrateFromSurrounding(ic, state);
}

void MsKeyEngine::deactivate(const fcitx::InputMethodEntry &entry, fcitx::InputContextEvent &event) {
    reset(entry, event);
    if (auto *ic = event.inputContext()) {
        if (auto *state = ic->propertyFor(&factory_)) {
            state->active = false;
        }
    }
}

void MsKeyEngine::reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    if (!ic) {
        return;
    }
    auto *state = ic->propertyFor(&factory_);
    if (!state || !state->handle) {
        return;
    }
    flushPreedit(ic, state);
    state->hydratedWord.clear();
    state->hydratedLen = 0;
    state->holdEnglishActive = false;
    resetHotkeyTracker(state);
    mskey_set_temporary_english(state->handle, false);
    mskey_reset_all(state->handle);
}

std::string MsKeyEngine::subMode(const fcitx::InputMethodEntry &, fcitx::InputContext &inputContext) {
    auto *state = inputContext.propertyFor(&factory_);
    if (!state || !state->handle || state->appBlacklisted || state->holdEnglishActive) {
        return "E";
    }
    return mskey_is_enabled(state->handle) ? "V" : "E";
}

void MsKeyEngine::keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &keyEvent) {
    auto *ic = keyEvent.inputContext();
    if (!ic) {
        return;
    }
    auto *state = ic->propertyFor(&factory_);
    if (!state || !state->handle) {
        return;
    }
    reloadConfigIfNeeded(state);

    const bool isRelease = keyEvent.isRelease();
    const fcitx::Key key = keyEvent.key();
    const fcitx::KeySym sym = key.sym();
    const fcitx::KeyStates states = key.states();

    if (matchesHoldEnglishSym(state->settings.holdEnglishKey, sym)) {
        if (!isRelease) {
            if (!state->holdEnglishActive) {
                state->holdEnglishActive = true;
                flushPreedit(ic, state);
                state->hydratedLen = 0;
                state->hydratedWord.clear();
                mskey_set_temporary_english(state->handle, true);
            }
        } else if (state->holdEnglishActive) {
            state->holdEnglishActive = false;
            mskey_set_temporary_english(state->handle, false);
        }
        return;
    }

    if (state->settings.quickCapslockSwitch && !isRelease && sym == FcitxKey_Caps_Lock
        && state->settings.holdEnglishKey != HoldEnglishKeyMode::CapsLock
        && !states.testAny(fcitx::KeyStates{
            fcitx::KeyState::Ctrl,
            fcitx::KeyState::Alt,
            fcitx::KeyState::Super,
            fcitx::KeyState::Shift,
        })) {
        resetHotkeyTracker(state);
        flushPreedit(ic, state);
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        bool next = !mskey_is_enabled(state->handle);
        state->preferredVietnamese = next;
        mskey_set_enabled(state->handle, next);
        ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
        keyEvent.filterAndAccept();
        return;
    }

    if (!isRelease && matchesKeyCombo(state->settings.undoLastWordHotkey, sym, states)) {
        if (state->appBlacklisted) {
            return;
        }
        if (handleUndoHotkey(ic, state)) {
            keyEvent.filterAndAccept();
        }
        return;
    }

    const bool isCtrlSym = (sym == FcitxKey_Control_L || sym == FcitxKey_Control_R);
    const bool isShiftSym = (sym == FcitxKey_Shift_L || sym == FcitxKey_Shift_R);
    const bool isGroupShift = (sym == FcitxKey_ISO_Next_Group || sym == FcitxKey_ISO_Prev_Group);
    if (key.isModifier() || isGroupShift) {
        if (state->settings.switchPreset == SwitchPresetMode::CtrlShift) {
            bool ctrlHeld = states.test(fcitx::KeyState::Ctrl) || (!isRelease && isCtrlSym);
            bool shiftHeld = states.test(fcitx::KeyState::Shift) || (!isRelease && (isShiftSym || isGroupShift));
            if (!isRelease && ctrlHeld && shiftHeld) {
                state->ctrlShiftPrimed = true;
            } else if (isRelease && (isCtrlSym || isShiftSym || isGroupShift) && state->ctrlShiftPrimed) {
                state->ctrlShiftPrimed = false;
                flushPreedit(ic, state);
                bool next = !mskey_is_enabled(state->handle);
                state->preferredVietnamese = next;
                mskey_set_enabled(state->handle, next);
                ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
            } else if (!ctrlHeld && !shiftHeld) {
                state->ctrlShiftPrimed = false;
            }
        } else if (state->settings.switchPreset == SwitchPresetMode::ShiftShift && isShiftSym) {
            auto now = std::chrono::steady_clock::now();
            bool otherMods = states.testAny(fcitx::KeyStates{
                fcitx::KeyState::Ctrl,
                fcitx::KeyState::Alt,
                fcitx::KeyState::Super,
            });
            if (!isRelease) {
                if (otherMods) {
                    state->shiftTainted = true;
                    state->shiftHeldForTap = false;
                    state->hasLastShiftTap = false;
                } else if (!state->shiftHeldForTap) {
                    state->shiftHeldForTap = true;
                    state->shiftPressAt = now;
                    state->shiftTainted = false;
                }
            } else {
                auto elapsedMs = std::chrono::duration_cast<std::chrono::milliseconds>(now - state->shiftPressAt).count();
                bool valid = !state->shiftTainted && !otherMods && state->shiftHeldForTap && elapsedMs <= 350;
                state->shiftHeldForTap = false;
                state->shiftTainted = false;
                if (valid) {
                    auto sincePrevMs = std::chrono::duration_cast<std::chrono::milliseconds>(now - state->lastShiftTapAt).count();
                    if (state->hasLastShiftTap && sincePrevMs <= 400) {
                        state->hasLastShiftTap = false;
                        flushPreedit(ic, state);
                        bool next = !mskey_is_enabled(state->handle);
                        state->preferredVietnamese = next;
                        mskey_set_enabled(state->handle, next);
                        ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
                    } else {
                        state->lastShiftTapAt = now;
                        state->hasLastShiftTap = true;
                    }
                } else {
                    state->hasLastShiftTap = false;
                }
            }
        }
        return;
    }

    state->ctrlShiftPrimed = false;
    state->shiftHeldForTap = false;
    state->hasLastShiftTap = false;
    state->shiftTainted = !isRelease;
    if (isRelease) {
        return;
    }

    if ((state->settings.switchPreset == SwitchPresetMode::AltZ
         && states.test(fcitx::KeyState::Alt) && !states.test(fcitx::KeyState::Ctrl)
         && (sym == 'z' || sym == 'Z'))
        || (state->settings.switchPreset == SwitchPresetMode::CtrlSpace
            && states.test(fcitx::KeyState::Ctrl)
            && !states.testAny(fcitx::KeyStates{fcitx::KeyState::Alt, fcitx::KeyState::Shift, fcitx::KeyState::Super})
            && (sym == FcitxKey_space || sym == FcitxKey_KP_Space))
        || (state->settings.switchPreset == SwitchPresetMode::Custom
            && matchesKeyCombo(state->settings.customSwitchHotkey, sym, states))) {
        flushPreedit(ic, state);
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        bool next = !mskey_is_enabled(state->handle);
        state->preferredVietnamese = next;
        mskey_set_enabled(state->handle, next);
        ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
        keyEvent.filterAndAccept();
        return;
    }

    if (state->appBlacklisted) {
        return;
    }

    if (states.testAny(fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt, fcitx::KeyState::Super})) {
        flushPreedit(ic, state);
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        mskey_reset_word(state->handle);
        return;
    }

    const bool usePreedit = state->settings.usePreedit
        && ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit);

    if (sym == FcitxKey_Escape) {
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        if (usePreedit && !state->preedit.empty()) {
            MsKeyAction act = mskey_process_escape(state->handle);
            if (act.kind == 1) {
                saveLastEscWord(state->handle);
                applyReplaceToUtf8(state->preedit, act.backspaces, act.text);
                flushPreedit(ic, state);
                forwardPendingCursorLeft(ic, state->handle);
                mskey_free_action(act);
                keyEvent.filterAndAccept();
                return;
            }
            mskey_free_action(act);
            flushPreedit(ic, state);
            mskey_reset_word(state->handle);
            return;
        }
        MsKeyAction act = mskey_process_escape(state->handle);
        if (act.kind == 1) {
            saveLastEscWord(state->handle);
            std::string text = act.text ? act.text : "";
            commitDirectReplace(ic, state, act.backspaces, text);
            forwardPendingCursorLeft(ic, state->handle);
            mskey_free_action(act);
            keyEvent.filterAndAccept();
            return;
        }
        mskey_free_action(act);
        mskey_reset_word(state->handle);
        return;
    }

    if (sym == FcitxKey_BackSpace) {
        if (usePreedit && !state->preedit.empty()) {
            MsKeyAction act = mskey_process_backspace(state->handle);
            if (act.kind == 1) {
                applyReplaceToUtf8(state->preedit, act.backspaces, act.text);
            } else {
                eraseLastCodepoints(state->preedit, 1);
            }
            mskey_free_action(act);
            updateClientPreedit(ic, state);
            keyEvent.filterAndAccept();
            return;
        }
        MsKeyAction act = mskey_process_backspace(state->handle);
        if (act.kind == 1) {
            state->hydratedLen = 0;
            state->hydratedWord.clear();
            std::string text = act.text ? act.text : "";
            commitDirectReplace(ic, state, act.backspaces, text);
            mskey_free_action(act);
            keyEvent.filterAndAccept();
            return;
        }
        mskey_free_action(act);
        if (state->hydratedLen > 0) {
            state->hydratedLen--;
            state->suppressSurroundingCount++;
            eraseLastCodepoints(state->hydratedWord, 1);
        } else {
            mskey_reset_word(state->handle);
        }
        return;
    }

    if (sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_Return || sym == FcitxKey_KP_Enter) {
        const uint32_t delim = (sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab) ? '\t' : '\n';
        const bool vnEnabled = mskey_is_enabled(state->handle);
        const bool activeTyping = !state->holdEnglishActive
            && (vnEnabled || (state->settings.macroEnabled && state->settings.macroWhenEnglish));
        if (activeTyping) {
            if (state->settings.hasClipboardMacro) {
                std::string clip = readClipboardText();
                mskey_set_clipboard_text(state->handle, clip.empty() ? nullptr : clip.c_str());
            }
            MsKeyAction act = mskey_process_key(state->handle, delim);
            if (act.kind == 1) {
                std::string text = act.text ? act.text : "";
                uint32_t bs = act.backspaces;
                mskey_free_action(act);
                if (usePreedit && !state->preedit.empty()) {
                    bool hasTrailing = !text.empty() && (text.back() == '\n' || text.back() == '\t');
                    char trailing = hasTrailing ? text.back() : '\0';
                    std::string body = hasTrailing ? text.substr(0, text.size() - 1) : text;
                    applyReplaceToUtf8(state->preedit, bs, body.c_str());
                    flushPreedit(ic, state);
                    if (hasTrailing) {
                        ic->forwardKey(fcitx::Key(trailing == '\n' ? FcitxKey_Return : FcitxKey_Tab));
                    }
                } else {
                    uint32_t del = state->hydratedLen > 0 ? std::min(bs, state->hydratedLen) : bs;
                    state->hydratedLen = 0;
                    state->hydratedWord.clear();
                    commitDirectReplace(ic, state, del, text);
                }
                forwardPendingCursorLeft(ic, state->handle);
                keyEvent.filterAndAccept();
                return;
            }
            mskey_free_action(act);
        } else {
            mskey_reset_word(state->handle);
        }
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        flushPreedit(ic, state);
        return;
    }

    if (isNavigationKeySym(sym)) {
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        flushPreedit(ic, state);
        mskey_reset_word(state->handle);
        return;
    }

    uint32_t cp = keySymToCodepoint(sym);
    if (cp == 0) {
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        flushPreedit(ic, state);
        mskey_reset_word(state->handle);
        return;
    }

    const bool vnEnabled = mskey_is_enabled(state->handle);
    const bool activeTyping = !state->holdEnglishActive
        && (vnEnabled || (state->settings.macroEnabled && state->settings.macroWhenEnglish));
    if (!activeTyping) {
        state->hydratedLen = 0;
        state->hydratedWord.clear();
        flushPreedit(ic, state);
        return;
    }

    const bool isDelim = mskey_is_active_delimiter(state->handle, cp);
    if (isDelim && state->settings.hasClipboardMacro) {
        std::string clip = readClipboardText();
        mskey_set_clipboard_text(state->handle, clip.empty() ? nullptr : clip.c_str());
    }

    MsKeyAction act = mskey_process_key(state->handle, cp);
    uint32_t hydrated = state->hydratedLen;
    std::string hydratedWord = state->hydratedWord;
    state->hydratedLen = 0;
    state->hydratedWord.clear();

    if (isDelim) {
        if (act.kind == 1) {
            std::string text = act.text ? act.text : "";
            uint32_t bs = act.backspaces;
            mskey_free_action(act);
            if (usePreedit && !state->preedit.empty()) {
                applyReplaceToUtf8(state->preedit, bs, text.c_str());
                flushPreedit(ic, state);
            } else {
                uint32_t del = hydrated > 0 ? std::min(bs, hydrated) : bs;
                commitDirectReplace(ic, state, del, text);
            }
            forwardPendingCursorLeft(ic, state->handle);
            keyEvent.filterAndAccept();
            return;
        }
        mskey_free_action(act);
        if (usePreedit && !state->preedit.empty()) {
            appendUtf8(state->preedit, cp);
            flushPreedit(ic, state);
            keyEvent.filterAndAccept();
        }
        return;
    }

    if (usePreedit) {
        if (act.kind == 1) {
            std::string text = act.text ? act.text : "";
            uint32_t bs = act.backspaces;
            mskey_free_action(act);
            if (hydrated > 0) {
                commitDirectReplace(ic, state, std::min(bs, hydrated), text);
                forwardPendingCursorLeft(ic, state->handle);
                keyEvent.filterAndAccept();
                return;
            }
            if (!state->preedit.empty()) {
                applyReplaceToUtf8(state->preedit, bs, text.c_str());
                updateClientPreedit(ic, state);
                keyEvent.filterAndAccept();
                return;
            }
            commitDirectReplace(ic, state, bs, text);
            forwardPendingCursorLeft(ic, state->handle);
            keyEvent.filterAndAccept();
            return;
        }
        mskey_free_action(act);
        if (hydrated > 0) {
            state->suppressSurroundingCount++;
            ic->deleteSurroundingText(-static_cast<int>(hydrated), hydrated);
            state->preedit = hydratedWord;
            appendUtf8(state->preedit, cp);
            updateClientPreedit(ic, state);
            keyEvent.filterAndAccept();
            return;
        }
        appendUtf8(state->preedit, cp);
        updateClientPreedit(ic, state);
        keyEvent.filterAndAccept();
        return;
    }

    if (act.kind == 1) {
        std::string text = act.text ? act.text : "";
        uint32_t bs = act.backspaces;
        mskey_free_action(act);
        commitDirectReplace(ic, state, bs, text);
        forwardPendingCursorLeft(ic, state->handle);
        keyEvent.filterAndAccept();
        return;
    }
    mskey_free_action(act);
}

fcitx::AddonInstance *MsKeyFactory::create(fcitx::AddonManager *manager) {
    return new MsKeyEngine(manager->instance());
}

}

using mskey::MsKeyFactory;
FCITX_ADDON_FACTORY(MsKeyFactory);
