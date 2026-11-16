use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use mskey_core::{AppBehavior, Config, Engine};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};

pub const WM_APP_TRAY: u32 = 0x8001;
pub const WM_APP_SYNC_UI: u32 = 0x8002;
pub const WM_APP_TOGGLE_VE: u32 = 0x8003;
pub const WM_APP_OPEN_CONVERT: u32 = 0x8004;
pub const WM_APP_OPEN_MAIN: u32 = 0x8005;
pub const WM_APP_SHOW_OSD: u32 = 0x8006;
pub const WM_APP_CONVERT_SELECTION: u32 = 0x8007;
pub const WM_APP_UPDATE_CARET_OSD: u32 = 0x8008;
pub const WM_APP_UPDATE_RESULT: u32 = 0x8009;

pub struct AppState {
    pub engine: Engine,
    pub main_hwnd: usize,
    pub fg_process: String,
    pub fg_class: String,
    pub fg_title: String,
    pub fg_is_us: bool,
    pub fg_hwnd: usize,
    pub last_typing_hwnd: usize,
    pub user_preferred_ve: bool,
    pub app_ve_memory: HashMap<String, bool>,
    pub suspend_typing_for_hotkey: bool,
    pub pending_ctrl_shift: bool,
    pub ctrl_down: bool,
    pub shift_down: bool,
    pub alt_down: bool,
    pub win_down: bool,
    pub l_ctrl_down: bool,
    pub r_ctrl_down: bool,
    pub l_shift_down: bool,
    pub r_shift_down: bool,
    pub l_alt_down: bool,
    pub r_alt_down: bool,
    pub l_win_down: bool,
    pub r_win_down: bool,
    pub caps_lock_on: bool,
    pub caps_key_down: bool,
}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

pub fn init_state(config: Config) {
    let pref = config.vietnamese_enabled;
    let mut engine = Engine::new(config);
    engine.set_clock_offset_secs(crate::platform_extra::local_utc_offset_secs());
    let caps_init = (unsafe { GetKeyState(VK_CAPITAL as i32) } & 1) != 0;
    let _ = STATE.set(Mutex::new(AppState {
        engine,
        main_hwnd: 0,
        fg_process: String::new(),
        fg_class: String::new(),
        fg_title: String::new(),
        fg_is_us: true,
        fg_hwnd: 0,
        last_typing_hwnd: 0,
        user_preferred_ve: pref,
        app_ve_memory: HashMap::new(),
        suspend_typing_for_hotkey: false,
        pending_ctrl_shift: false,
        ctrl_down: false,
        shift_down: false,
        alt_down: false,
        win_down: false,
        l_ctrl_down: false,
        r_ctrl_down: false,
        l_shift_down: false,
        r_shift_down: false,
        l_alt_down: false,
        r_alt_down: false,
        l_win_down: false,
        r_win_down: false,
        caps_lock_on: caps_init,
        caps_key_down: false,
    }));
}

pub fn with_state<R>(f: impl FnOnce(&mut AppState) -> R) -> R {
    let lock = STATE.get().expect("state initialized");
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

impl AppState {
    pub fn main_window(&self) -> HWND {
        self.main_hwnd as HWND
    }

    pub fn current_window_behavior(&self) -> AppBehavior {
        if self.fg_process.is_empty() {
            AppBehavior::Default
        } else {
            self.engine
                .config()
                .behavior_for_window(&self.fg_process, &self.fg_title)
        }
    }

    pub fn is_blacklisted(&self) -> bool {
        self.current_window_behavior() == AppBehavior::Blacklist
    }

    pub fn effective_vietnamese(&self) -> bool {
        self.engine.is_vietnamese_active()
    }

    pub fn set_user_vietnamese(&mut self, enabled: bool) {
        self.user_preferred_ve = enabled;
        self.engine.set_vietnamese_enabled(enabled);
        if self.engine.config().smart_app_switch
            && !self.fg_process.is_empty()
            && self.current_window_behavior() == AppBehavior::Default
        {
            self.app_ve_memory
                .insert(self.fg_process.to_ascii_lowercase(), enabled);
        }
    }

    fn save_current_app_ve(&mut self, prev_behavior: AppBehavior) {
        if !self.fg_process.is_empty()
            && self.engine.config().smart_app_switch
            && prev_behavior == AppBehavior::Default
        {
            self.app_ve_memory.insert(
                self.fg_process.to_ascii_lowercase(),
                self.engine.config().vietnamese_enabled,
            );
        }
    }

    fn apply_window_behavior(&mut self) {
        match self.current_window_behavior() {
            AppBehavior::ForceEnglish => {
                self.engine.set_vietnamese_enabled(false);
            }
            AppBehavior::ForceVietnamese => {
                self.engine.set_vietnamese_enabled(true);
            }
            AppBehavior::Blacklist => {}
            AppBehavior::Default => {
                let target =
                    if self.engine.config().smart_app_switch && !self.fg_process.is_empty() {
                        self.app_ve_memory
                            .get(&self.fg_process.to_ascii_lowercase())
                            .copied()
                            .unwrap_or(self.user_preferred_ve)
                    } else {
                        self.user_preferred_ve
                    };
                self.engine.set_vietnamese_enabled(target);
            }
        }
    }

    pub fn reapply_current_window_rule(&mut self) {
        self.apply_window_behavior();
    }

    pub fn update_foreground(
        &mut self,
        hwnd: usize,
        process: String,
        class_name: String,
        title: String,
        is_us: bool,
    ) {
        let same_app = self.fg_process.eq_ignore_ascii_case(&process) && !process.is_empty();
        let prev_behavior = self.current_window_behavior();
        self.fg_hwnd = hwnd;
        if hwnd != 0
            && hwnd != self.main_hwnd
            && !matches!(
                class_name.as_str(),
                "Shell_TrayWnd"
                    | "NotifyIconOverflowWindow"
                    | "TopLevelWindowForOverflowXamlIsland"
                    | "MSKeyConvertPopup"
                    | "MSKeyCaretHUD"
                    | "MSKeySwitchOSD"
            )
        {
            self.last_typing_hwnd = hwnd;
        }
        self.fg_class = class_name;
        self.fg_is_us = is_us;
        self.engine.reset_all();
        if same_app {
            self.fg_title = title;
            if self.current_window_behavior() != prev_behavior {
                self.save_current_app_ve(prev_behavior);
                self.apply_window_behavior();
            }
            return;
        }
        self.save_current_app_ve(prev_behavior);
        self.fg_process = process;
        self.fg_title = title;
        self.apply_window_behavior();
    }

    pub fn update_foreground_title(&mut self, title: String) -> bool {
        if self.fg_title == title || self.fg_process.is_empty() {
            return false;
        }
        let prev_behavior = self.current_window_behavior();
        let prev_active = self.engine.is_vietnamese_active();
        let prev_blacklisted = prev_behavior == AppBehavior::Blacklist;
        let new_behavior = self
            .engine
            .config()
            .behavior_for_window(&self.fg_process, &title);
        if new_behavior == prev_behavior {
            self.fg_title = title;
            return false;
        }
        self.save_current_app_ve(prev_behavior);
        self.fg_title = title;
        self.apply_window_behavior();
        prev_active != self.engine.is_vietnamese_active()
            || prev_blacklisted != self.is_blacklisted()
    }
}
