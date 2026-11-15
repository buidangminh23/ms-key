#ifndef MSKEY_FCITX5_H
#define MSKEY_FCITX5_H

#include <chrono>
#include <cstdint>
#include <memory>
#include <string>

#include <fcitx-utils/handlertable.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>

#include "mskey_core.h"

namespace mskey {

enum class HoldEnglishKeyMode {
    None,
    RightShift,
    LeftShift,
    RightAlt,
    RightCtrl,
    CapsLock,
};

enum class SwitchPresetMode {
    CtrlShift,
    AltZ,
    CtrlSpace,
    ShiftShift,
    Custom,
};

struct KeyCombo {
    bool ctrl = false;
    bool alt = false;
    bool shift = false;
    bool winCmd = false;
    uint32_t keyCode = 0;
};

struct ParsedSettings {
    bool vietnameseEnabled = true;
    bool usePreedit = true;
    bool macroEnabled = true;
    bool macroWhenEnglish = false;
    bool macroTriggerTabOnly = false;
    bool smartAppSwitch = true;
    bool quickCapslockSwitch = false;
    bool reeditSurroundingWord = true;
    bool hasClipboardMacro = false;
    std::string syncConfigPath;
    HoldEnglishKeyMode holdEnglishKey = HoldEnglishKeyMode::None;
    SwitchPresetMode switchPreset = SwitchPresetMode::CtrlShift;
    KeyCombo customSwitchHotkey;
    KeyCombo undoLastWordHotkey;
};

class MsKeyState : public fcitx::InputContextProperty {
public:
    MsKeyState();
    ~MsKeyState() override;

    MsKeyState(const MsKeyState &) = delete;
    MsKeyState &operator=(const MsKeyState &) = delete;

    MsKeyHandle *handle = nullptr;
    ParsedSettings settings;
    std::string preedit;
    std::string hydratedWord;
    uint32_t hydratedLen = 0;
    uint32_t suppressSurroundingCount = 0;
    int64_t configMtimeNs = -1;
    bool active = false;
    bool appBlacklisted = false;
    bool preferredVietnamese = true;
    bool holdEnglishActive = false;
    bool ctrlShiftPrimed = false;
    bool shiftTainted = false;
    bool shiftHeldForTap = false;
    std::chrono::steady_clock::time_point shiftPressAt;
    std::chrono::steady_clock::time_point lastShiftTapAt;
    bool hasLastShiftTap = false;
};

class MsKeyEngine : public fcitx::InputMethodEngineV2 {
public:
    explicit MsKeyEngine(fcitx::Instance *instance);

    void activate(const fcitx::InputMethodEntry &entry, fcitx::InputContextEvent &event) override;
    void deactivate(const fcitx::InputMethodEntry &entry, fcitx::InputContextEvent &event) override;
    void reset(const fcitx::InputMethodEntry &entry, fcitx::InputContextEvent &event) override;
    void keyEvent(const fcitx::InputMethodEntry &entry, fcitx::KeyEvent &keyEvent) override;
    std::string subMode(const fcitx::InputMethodEntry &entry, fcitx::InputContext &inputContext) override;

private:
    void reloadConfigIfNeeded(MsKeyState *state);
    void applyAppPolicy(fcitx::InputContext *ic, MsKeyState *state);
    void hydrateFromSurrounding(fcitx::InputContext *ic, MsKeyState *state);
    void updateClientPreedit(fcitx::InputContext *ic, MsKeyState *state);
    void flushPreedit(fcitx::InputContext *ic, MsKeyState *state);
    void commitDirectReplace(
        fcitx::InputContext *ic,
        MsKeyState *state,
        uint32_t backspaces,
        const std::string &text
    );
    void forwardPendingCursorLeft(fcitx::InputContext *ic, MsKeyHandle *handle);
    void resetHotkeyTracker(MsKeyState *state);
    bool handleUndoHotkey(fcitx::InputContext *ic, MsKeyState *state);

    fcitx::Instance *instance_;
    fcitx::FactoryFor<MsKeyState> factory_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> surroundingWatcher_;
};

class MsKeyFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override;
};

}

#endif
