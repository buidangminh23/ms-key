#![cfg_attr(not(test), windows_subsystem = "windows")]

mod crash;
mod dialog_convert;
mod dialog_custom;
mod dialog_helpers;
mod dialogs;
mod hook;
mod hook_send;
mod platform;
mod platform_extra;
mod state;
mod theme;
mod tray;
mod ui;
mod ui_actions;
mod ui_controls;
mod ui_extras;
mod ui_osd;
mod ui_paint;
mod ui_popup;
mod uia;
mod update_win;

use std::env;

use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

use crate::hook::install_hooks;
use crate::platform::{
    acquire_single_instance_for, is_elevated, load_config, relaunch_as_admin,
    signal_existing_instance,
};
use crate::state::init_state;
use crate::ui::create_and_run_main;

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let args: Vec<String> = env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--verify-smoke") {
        let out = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("smoke_layout.txt");
        let cfg = load_config();
        init_state(cfg);
        create_and_run_main(true, Some(out));
        return;
    }

    crate::crash::install(true);
    let relaunching = args.iter().any(|a| a == "--relaunch");
    let want_show = args.iter().any(|a| a == "--show");
    let mut instance_guard = acquire_single_instance_for(relaunching);
    if instance_guard.is_none() {
        signal_existing_instance();
        return;
    }

    let cfg = load_config();
    if cfg.run_as_admin && !is_elevated() && !args.iter().any(|a| a == "--no-relaunch") {
        instance_guard.take();
        if relaunch_as_admin(want_show) {
            return;
        }
        instance_guard = acquire_single_instance_for(false);
        if instance_guard.is_none() {
            signal_existing_instance();
            return;
        }
    }
    let _instance_guard = instance_guard;

    let show_gui = cfg.show_on_startup || want_show;
    init_state(cfg);
    crate::uia::start();
    let _hooks = install_hooks();
    create_and_run_main(show_gui, None);
}
