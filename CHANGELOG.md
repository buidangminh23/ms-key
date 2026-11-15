# Changelog

All notable changes to MS Key are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [0.1.3] - 2026-11-15

### Changed

- The macOS menu bar now shows the same single large letter as Windows: a red **V** for Vietnamese and a blue **E** for English.
- macOS install instructions now lead with the Terminal installer. A browser-downloaded DMG is quarantined and, because MS Key is not notarized, macOS refuses to open it; the README and the disk image's `Install.txt` give the one-line `xattr` fix.

### Added

- Click the macOS menu bar letter once to switch between Vietnamese and English. Right-click or Control-click opens the menu.

### Known limitations

- Linux: GNOME and other IBus panels draw the `V`/`E` mode symbol themselves in monochrome text, and clicking it opens the input-source menu, so the colored letter and one-click switching are not available there. Switch with the configured hotkey or the **Chế độ** menu item.

### Downloads

- macOS 13+ (Intel and Apple Silicon): run `curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash`. The DMG `MSKey-0.1.3-macos-universal.dmg` also works after `xattr -dr com.apple.quarantine /Applications/MSKey.app`.
- Windows 10/11: `MSKey-Setup-0.1.3-windows-x64.exe` or `MSKey-0.1.3-windows-x64-portable.zip`.
- Linux: `mskey_0.1.3_amd64.deb` or `MSKey-0.1.3-linux-amd64.tar.gz`.

All downloads include SHA-256 checksums.

## [0.1.2] - 2026-11-15

### Changed

- The Windows tray icon is now a single large letter: a red **V** for Vietnamese and a blue **E** for English, sized like the system volume icon.

### Added

- Click the Windows tray icon once to switch between Vietnamese and English. Double-clicking still opens the settings window and keeps the current mode.

### Downloads

- macOS 13+ (Intel and Apple Silicon): `MSKey-0.1.2-macos-universal.dmg`. Open it and drag MSKey.app into Applications.
- Windows 10/11: `MSKey-Setup-0.1.2-windows-x64.exe` or `MSKey-0.1.2-windows-x64-portable.zip`.
- Linux: `mskey_0.1.2_amd64.deb` or `MSKey-0.1.2-linux-amd64.tar.gz`.

All downloads include SHA-256 checksums.

## [0.1.1] - 2026-11-15

### Added

- MS Key branding across Windows executable, window, tray and installer icons; macOS app, menu bar and DMG; Linux launcher, IBus and settings; and the project README.
- Brief nonblocking logo entrance animations when opening settings, respecting each system's reduced-motion preference.
- Live byte-based download progress in the npm installer, with plain output for redirected terminals and `MSKEY_REDUCE_MOTION=1`.
- macOS Terminal download progress and activity indicators while unpacking, verifying and installing the app.
- Branded Windows setup pages and smooth native installation progress.

### Fixed

- The npm macOS installer now checks the installer script against `SHA256SUMS` before executing it.

### Downloads

- macOS 13+ (Intel and Apple Silicon): `MSKey-0.1.1-macos-universal.dmg`. Open it and drag MSKey.app into Applications.
- Windows 10/11: `MSKey-Setup-0.1.1-windows-x64.exe` or `MSKey-0.1.1-windows-x64-portable.zip`.
- Linux: `mskey_0.1.1_amd64.deb` or `MSKey-0.1.1-linux-amd64.tar.gz`.

All downloads include SHA-256 checksums. macOS remains ad-hoc signed and not notarized; first launch may require **System Settings > Privacy & Security > Open Anyway**, followed by Accessibility permission.

## [0.1.0] - 2026-11-15

First public release of MS Key, a lightweight Vietnamese Unicode input method for Windows, Linux and macOS built on one shared Rust engine.

### Download

| System | File | How to install |
|---|---|---|
| Windows 10/11 (x64) | `MSKey-Setup-0.1.0-windows-x64.exe` | Run the installer. It installs for the current user, no administrator rights needed. |
| Windows (portable) | `MSKey-0.1.0-windows-x64-portable.zip` | Unzip and run `MSKey.exe`. |
| Ubuntu / Debian (amd64) | `mskey_0.1.0_amd64.deb` | `sudo apt install ./mskey_0.1.0_amd64.deb`, then `ibus restart` and add **MS Key (Tiếng Việt)** under Settings > Keyboard > Input Sources. |
| Other Linux (amd64) | `MSKey-0.1.0-linux-amd64.tar.gz` | Extract and run `./install.sh` (needs GTK 3 and IBus). |
| macOS 13+ (Apple Silicon and Intel) | `MSKey-0.1.0-macos-universal.dmg` | Open the disk image, drag **MSKey.app** into **Applications**, eject the image and open MS Key from Applications. |
| macOS (Terminal installer) | `MSKey-0.1.0-macos-universal.zip` | `curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh \| bash` |

Every file is listed in `SHA256SUMS`.

### Features

- Input methods: Telex, VNI, VIQR, Simple Telex and a fully custom keymap.
- Charsets: Unicode (NFC), Unicode composite (NFD), TCVN3 (ABC) and VNI Windows.
- Modern (`hoà`, `thuý`) or classic (`hòa`, `thúy`) tone placement, free tone placement and spelling check.
- Automatic restore of English and code words (`windows`, `function`, `class`), with Esc to restore the raw keys of the current word.
- Quick typing: `w` → `ư`, `[` → `ơ`, `]` → `ư`, double consonants (`cc` → `ch`, `nn` → `ng`), start and end consonant shortcuts.
- Macros with `{date}`, `{time}` and `{datetime}`, usable even when Vietnamese is off.
- Clipboard converter between charsets, plus case conversion and diacritic removal.
- Per-app rules: always English, always Vietnamese or ignore, and remembering V/E per app.
- Import settings from EVKey `setting.tree`.
- Windows: Win32 settings panel with light/dark mode, V/E tray icon, low-level keyboard hook and a browser suggestion fix.
- Linux: native IBus engine (preedit or direct mode, X11 and Wayland) and a GTK 3 settings panel.
- macOS: menu bar app with V/E badge, `CGEventTap` hook and SwiftUI settings.

### Fixed

- Typing `dd`, `aa`, `ee`, `oo`, `aw`, `ow` or `uw` on its own now keeps `đ`, `â`, `ê`, `ô`, `ă`, `ơ`, `ư` instead of restoring the raw keys.

### Verification

- 36 Rust tests and 3 Swift tests pass on Windows, Linux and macOS; `cargo clippy -D warnings` is clean on all three.
- The release workflow builds every package, installs it, launches the settings panel in smoke-test mode and uninstalls it on GitHub-hosted Windows, Ubuntu 22.04 and macOS runners before publishing.

### Known limitations

- The Windows installer and macOS app are not code-signed by a paid certificate. Windows SmartScreen may warn on first run (choose **More info > Run anyway**). The macOS app is ad-hoc signed and not notarized. If Gatekeeper blocks the first launch after installing from the DMG, open **System Settings > Privacy & Security > Open Anyway** for MS Key and confirm the launch. The Terminal installer remains available as an alternative.
- macOS needs Accessibility permission. After every update, remove the old MS Key entry in System Settings > Privacy & Security > Accessibility and allow it again.
- The Linux packages target amd64 with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+).
- Real typing through each system's keyboard hook was tested by hand only on the developer's machines; please report problems in GitHub Issues.

[0.1.0]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.0
[0.1.1]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.1
[0.1.2]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.2
[0.1.3]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.3
