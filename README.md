# MS Key

Lightweight, open-source cross-platform Vietnamese Unicode input method for Windows, Linux, and macOS, modeled after EVKey, OpenKey, Gõ Nhanh, and IBus Bamboo.

## Features

- Shared pure-Rust core engine (`mskey-core`) with zero OS dependencies:
  - Input methods: Telex, VNI, VIQR, Simple Telex, and Custom Keymap.
  - Charsets: Unicode (NFC), TCVN3 (ABC), VNI Windows, Unicode NFD.
  - Modern (`oà, uý`) and classic (`òa, úy`) tone placement, free tone placement, spelling validation, automatic English/code word restoration, Esc raw-word restore, macro expansion, clipboard encoding/case conversion, EVKey `setting.tree` import, and per-app V/E rules & blacklist.
- Windows (`mskey-win`): pure Rust + Win32 native EVKey-style UI (`560x568`) with Light/Dark mode, DPI awareness, dynamic `V`/`E` tray icon, `WH_KEYBOARD_LL` hook, browser suggestion fix, and admin elevation.
- Linux (`mskey-linux`): native GLib/GIO D-Bus `org.freedesktop.IBus` engine (`--ibus`, supporting both Wayland/X11 Preedit mode and Direct Surrounding/ForwardKey compatible mode) + native GTK3 Light/Dark setup UI (`560x568` style panel, macro/app/custom-keymap dialogs, clipboard converter `--convert`, XDG autostart, EVKey `setting.tree` import).
- macOS (`macos/MSKeyMac` via `mskey-ffi` static C ABI library): native Swift 6 / SwiftUI + AppKit menu bar app with `V`/`E` badge, `CGEventTap` hook, browser/Spotlight autofill fix, Light/Dark/System appearance, `SMAppService` autostart, and clipboard converter.

## Setup & Build

### Windows

```powershell
cargo test --workspace
cargo build --release -p mskey-win
```

### Linux (Ubuntu/Debian/Fedora/Arch with GTK3 + IBus)

```bash
sudo apt install libgtk-3-dev pkg-config ibus
cargo test -p mskey-core -p mskey-ffi -p mskey-linux
bash scripts/install-linux.sh
```

Then enable **MS Key (Tiếng Việt)** in `ibus-setup` or desktop Region & Language / Input Sources.

### macOS (Apple Silicon / Intel)

```bash
cargo test -p mskey-core -p mskey-ffi
bash scripts/build-macos.sh
open dist/MSKey.app
```

Grant Accessibility permission when prompted so `CGEventTap` can process system-wide typing.

## Repository Structure

```text
MS Key/
├── Cargo.toml
├── .github/workflows/ci.yml
├── crates/
│   ├── mskey-core/     # Pure Rust IME engine + charset converter + EVKey importer
│   ├── mskey-ffi/      # C ABI bridge + mskey_core.h + module.modulemap
│   ├── mskey-win/      # Windows Win32 hook, tray, and native EVKey UI
│   └── mskey-linux/    # Linux IBus D-Bus engine + GTK3 native EVKey UI
├── data/               # IBus component XML and desktop entry
├── macos/MSKeyMac/     # Native Swift 6 / SwiftUI + AppKit + CGEventTap app
└── scripts/            # install-linux.sh, build-macos.sh
```
