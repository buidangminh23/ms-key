pub mod config;
pub mod converter;
pub mod converter_extra;
pub mod dict;
pub mod engine;
mod engine_helpers;
mod engine_quick;
mod engine_roles;
pub mod syllable;
mod syllable_rimes;
pub mod tables;
pub mod transform;
pub mod update;

pub use config::{
    AppBehavior, AppRule, Charset, Config, ConfigProfile, CustomKeyMap, HoldEnglishKey,
    HotkeyCombo, InputMethod, MacroEntry, Mechanism, SwitchPreset, ThemeMode,
};
pub use converter::{CaseTransform, apply_case_transform, convert_text};
pub use converter_extra::{
    ConvertEncoding, convert_with_encoding, detect_charset, detect_convert_encoding,
    normalize_tone_style, strip_vietnamese_diacritics,
};
pub use engine::{Engine, EngineAction};
