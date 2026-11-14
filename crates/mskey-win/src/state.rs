use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use mskey_core::{AppBehavior, Config, Engine};
use windows_sys::Win32::Foundation::HWND;

pub const WM_APP_TRAY: u32 = 0x8001;
pub const WM_APP_SYNC_UI: u32 = 0x8002;
pub const WM_APP_TOGGLE_VE: u32 = 0x8003;
pub const WM_APP_OPEN_CONVERT: u32 = 0x8004;
pub const WM_APP_OPEN_MAIN: u32 = 0x8005;

pub struct AppState {
    pub engine: Engine,
    pub main_hwnd: usize,
    pub fg_process: String,
    pub fg_class: String,
    pub fg_is_us: bool,
    pub fg_hwnd: usize,
    pub user_preferred_ve: bool,
    pub app_ve_memory: HashMap<String, bool>,
    pub suspend_typing_for_hotkey: bool,
    pub pending_ctrl_shift: bool,
    pub ctrl_down: bool,
    pub shift_down: bool,
    pub alt_down: bool,
    pub win_down: bool,
}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

pub fn init_state(config: Config) {
    let pref = config.vietnamese_enabled;
    let _ = STATE.set(Mutex::new(AppState {
        engine: Engine::new(config),
        main_hwnd: 0,
        fg_process: String::new(),
        fg_class: String::new(),
        fg_is_us: true,
        fg_hwnd: 0,
        user_preferred_ve: pref,
        app_ve_memory: HashMap::new(),
        suspend_typing_for_hotkey: false,
        pending_ctrl_shift: false,
        ctrl_down: false,
        shift_down: false,
        alt_down: false,
        win_down: false,
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

    pub fn is_blacklisted(&self) -> bool {
        if self.fg_process.is_empty() {
            return false;
        }
        self.engine.config().behavior_for_app(&self.fg_process) == AppBehavior::Blacklist
    }

    pub fn set_user_vietnamese(&mut self, enabled: bool) {
        self.user_preferred_ve = enabled;
        self.engine.set_vietnamese_enabled(enabled);
        if self.engine.config().smart_app_switch && !self.fg_process.is_empty() {
            let b = self.engine.config().behavior_for_app(&self.fg_process);
            if b == AppBehavior::Default {
                self.app_ve_memory
                    .insert(self.fg_process.to_ascii_lowercase(), enabled);
            }
        }
    }

    pub fn update_foreground(
        &mut self,
        hwnd: usize,
        process: String,
        class_name: String,
        is_us: bool,
    ) {
        let same_app = self.fg_process.eq_ignore_ascii_case(&process) && !process.is_empty();
        self.fg_hwnd = hwnd;
        self.fg_class = class_name;
        self.fg_is_us = is_us;
        self.engine.reset_all();
        if same_app {
            return;
        }
        if !self.fg_process.is_empty() && self.engine.config().smart_app_switch {
            let prev_behavior = self.engine.config().behavior_for_app(&self.fg_process);
            if prev_behavior == AppBehavior::Default {
                self.app_ve_memory.insert(
                    self.fg_process.to_ascii_lowercase(),
                    self.engine.config().vietnamese_enabled,
                );
            }
        }
        self.fg_process = process;
        let behavior = if self.fg_process.is_empty() {
            AppBehavior::Default
        } else {
            self.engine.config().behavior_for_app(&self.fg_process)
        };
        match behavior {
            AppBehavior::ForceEnglish => {
                self.engine.set_vietnamese_enabled(false);
            }
            AppBehavior::ForceVietnamese => {
                self.engine.set_vietnamese_enabled(true);
            }
            AppBehavior::Blacklist => {}
            AppBehavior::Default => {
                let target = if self.engine.config().smart_app_switch && !self.fg_process.is_empty()
                {
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
}
