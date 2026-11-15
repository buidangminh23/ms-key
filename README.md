# MS Key

[![CI](https://github.com/buidangminh23/ms-key/actions/workflows/ci.yml/badge.svg)](https://github.com/buidangminh23/ms-key/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/buidangminh23/ms-key)](https://github.com/buidangminh23/ms-key/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

MS Key is a lightweight, open-source Vietnamese input method for **Windows, Linux and macOS**. One Rust engine does the typing on all three systems, so Telex, VNI and the rest behave the same everywhere. It follows the habits of EVKey, OpenKey, Gõ Nhanh and IBus Bamboo.

## Install

Download the files from the [latest release](https://github.com/buidangminh23/ms-key/releases/latest). Every file is listed with its checksum in `SHA256SUMS`.

With Node.js 18 or newer you can also let npm pick the right file for your system, verify it and run it:

```bash
npx @minhspark/ms-key
```

Add `--download <folder>` to only download and verify the installer.

### Windows 10/11

Run `MSKey-Setup-<version>-windows-x64.exe`. It installs for your user only and needs no administrator rights. MS Key then sits in the tray as **V** (Vietnamese) or **E** (English).

Windows SmartScreen may warn because the installer is not code-signed: choose **More info > Run anyway**. You can also unzip `MSKey-<version>-windows-x64-portable.zip` and run `MSKey.exe` without installing.

To remove it, use **Settings > Apps > Installed apps > MS Key > Uninstall**. Your settings in `%APPDATA%\MSKey` are kept.

### Ubuntu / Debian

```bash
sudo apt install ./mskey_<version>_amd64.deb
ibus restart
```

Then add **MS Key (Tiếng Việt)** in **Settings > Keyboard > Input Sources** (or `ibus-setup`). Remove it with `sudo apt remove mskey`.

On other distributions with GTK 3 and IBus, extract `MSKey-<version>-linux-amd64.tar.gz` and run `./install.sh` (and `./uninstall.sh` to remove). The prebuilt binaries need amd64 and glibc 2.35 or newer.

### macOS 13+ (Apple Silicon and Intel)

```bash
curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash
```

The installer checks the download against `SHA256SUMS`, verifies the app signature and installs `MSKey.app` into `/Applications` (or `~/Applications`). Use this command rather than a browser download: the app is not notarized, so a zip downloaded with a browser is blocked by Gatekeeper.

On first launch, allow MS Key in **System Settings > Privacy & Security > Accessibility**. After each update, remove the old MS Key entry there and allow it again.

## Quick start

- Press **Ctrl + Shift** to switch between Vietnamese (**V**) and English (**E**). Alt + Z, Ctrl + Space, Shift + Shift or your own shortcut can be chosen instead.
- Open the settings panel from the tray icon (Windows), the IBus menu or app launcher (Linux), or the menu bar icon (macOS).

| You type (Telex) | You get | You type (VNI) | You get |
|---|---|---|---|
| `tieesng vieejt` | tiếng việt | `tie61ng vie65t` | tiếng việt |
| `nguwowfi` | người | `ngu7o72i` | người |
| `dduowcj` | được | `d9uo75c` | được |
| `hoaf` | hoà | `hoa2` | hoà |
| `windows` | windows | | |

Press **Esc** right after a word to undo the Vietnamese marks and get back the keys you typed.

## Features

**Typing**

- Telex, VNI, VIQR, Simple Telex and a fully custom keymap.
- Modern (`hoà`, `thuý`) or classic (`hòa`, `thúy`) tone placement, and free tone placement anywhere in the word.
- Spelling check, so invalid Vietnamese syllables are left alone.
- English and code words such as `windows`, `function` or `class` are restored automatically. You can add your own words to a personal dictionary.
- Quick typing: `w` → `ư`, `[` → `ơ`, `]` → `ư`, `cc` → `ch`, `nn` → `ng`, `f` → `ph` at the start of a word, and more.
- Optional capitalisation at the start of a sentence.

**Output**

- Unicode (NFC), Unicode composite (NFD), TCVN3 (ABC) and VNI Windows.
- Clipboard converter between these charsets, with case conversion and diacritic removal (Ctrl + Shift + F6 by default).

**Macros**

- Short words that expand into longer text, for example `vn` → `Việt Nam`.
- Placeholders `{date}`, `{time}` and `{datetime}`.
- Can stay active while Vietnamese is off.

**Per-app behaviour**

- Remember V or E for each app automatically.
- Rules per app: always English, always Vietnamese, or ignore the app entirely.
- Fixes for browser address bar suggestions and terminals.

**Settings**

- Import an existing EVKey `setting.tree`.
- Light, dark or system theme; start with the system.
- Settings are stored as JSON: `%APPDATA%\MSKey\config.json` on Windows and `~/.config/mskey/config.json` on Linux and macOS.

## How it works on each system

| | Windows | Linux | macOS |
|---|---|---|---|
| Key capture | `WH_KEYBOARD_LL` keyboard hook | IBus engine over D-Bus (preedit or direct mode, X11 and Wayland) | `CGEventTap` |
| Settings UI | Native Win32 | GTK 3 | SwiftUI + AppKit menu bar app |
| Code | `crates/mskey-win` | `crates/mskey-linux` | `macos/MSKeyMac` via `crates/mskey-ffi` |

All three call the same engine in `crates/mskey-core`, which has no OS dependencies.

## Build from source

You need Rust 1.90 or newer.

**Windows**

```powershell
cargo test --workspace
cargo build --release -p mskey-win
```

The app is `target\release\mskey-win.exe`. Run it with `--show` to open the settings panel immediately.

**Linux** (GTK 3 and IBus development files)

```bash
sudo apt install libgtk-3-dev pkg-config ibus
cargo test -p mskey-core -p mskey-ffi -p mskey-linux
bash scripts/install-linux.sh
```

`install-linux.sh` builds and installs into `/usr/local`. Run `ibus restart` afterwards.

**macOS** (Xcode or the Swift 6 toolchain)

```bash
cargo test -p mskey-core -p mskey-ffi
bash scripts/build-macos.sh
open dist/MSKey.app
```

## Packaging and releases

| Script | Output |
|---|---|
| `scripts/package-windows.ps1` | NSIS installer and portable zip (needs NSIS) |
| `scripts/package-linux.sh` | `.deb` and `.tar.gz` |
| `scripts/package-macos.sh` | Universal ad-hoc signed `MSKey.app` zip and `install-macos.sh` |

Pushing a tag such as `v0.1.0` runs `.github/workflows/release.yml`. It builds every package on its own runner, installs it, launches the settings panel in smoke-test mode, uninstalls it, and then publishes the release with `SHA256SUMS` and the matching section of [CHANGELOG.md](CHANGELOG.md). The tag must match the version in `Cargo.toml`.

## Repository layout

```text
crates/
  mskey-core/     Typing engine, charset converter, dictionaries, EVKey importer
  mskey-ffi/      C ABI used by the macOS app (mskey_core.h)
  mskey-win/      Windows hook, tray icon and settings UI
  mskey-linux/    IBus engine and GTK settings UI
macos/MSKeyMac/   Swift menu bar app
data/             IBus component and desktop entry
scripts/          Build, install and packaging scripts
```

## Contributing

Bug reports and pull requests are welcome in [Issues](https://github.com/buidangminh23/ms-key/issues). Please run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` before opening a pull request.

## License

MS Key is released under the [MIT License](LICENSE).

The Vietnamese and English word lists in `crates/mskey-core/data` come from [Gõ Nhanh](https://github.com/khaphanspace/gonhanh.org) under the BSD 3-Clause License; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
