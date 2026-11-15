#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]

pub mod config_store;
pub mod ibus_logic;
pub mod keyval;
pub mod linux_extra;

#[cfg(target_os = "linux")]
pub mod gio_ffi;
#[cfg(target_os = "linux")]
pub mod gtk_dialogs;
#[cfg(target_os = "linux")]
pub mod gtk_branding;
#[cfg(target_os = "linux")]
pub mod gtk_dialogs_edit;
#[cfg(target_os = "linux")]
pub mod gtk_extras;
#[cfg(target_os = "linux")]
pub mod gtk_ffi;
#[cfg(target_os = "linux")]
pub mod gtk_helpers;
#[cfg(target_os = "linux")]
pub mod gtk_theme;
#[cfg(target_os = "linux")]
pub mod gtk_ui;
#[cfg(target_os = "linux")]
pub mod gtk_ui_tabs;
#[cfg(target_os = "linux")]
pub mod ibus_dbus;
#[cfg(target_os = "linux")]
pub mod ibus_variant;
