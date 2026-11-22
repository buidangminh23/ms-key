# Changelog

All notable changes to MS Key are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [0.2.13] - 2026-11-22

### Fixed

- On Linux, MS Key no longer switches itself off after login or when IBus restarts. MS Key now finishes setting up before it announces itself to IBus, so GNOME can select it right away instead of falling back to the English (US) layout.

### macOS installation

```sh
curl -fsSL https://github.com/buidangminh23/ms-key/releases/download/v0.2.13/install-macos.sh | bash
```

## [0.2.12] - 2026-11-22

### Fixed

- On macOS, Backspace right after a word boundary goes back into the word MS Key just typed. It no longer re-reads the word from the app through Accessibility, which in Chromium and Electron apps (Claude, Zalo, Chrome, VS Code) can lag behind the screen and make later keystrokes delete or replace the wrong letters. The Accessibility read is kept for words typed before MS Key saw them, and only after typing has paused for 0.6 s.
- On macOS, a focus notification from the app no longer drops the word being typed. MS Key keeps the last confirmed input field while it re-reads focus, so tone keys typed during that moment still apply. Secure Input still bypasses composition.
- On Linux, checking for and installing an update can no longer hang. Every step has a time limit, and the installer is stopped after 15 minutes with a message instead of waiting forever.

### Changed

- On Linux, the Updates section has one button and one status line. It shows “Cập nhật lên vX” when a newer release exists and installs it in place, with no pop-up dialogs. Checking on start only updates the status line.
- The Linux `.deb` installs `/usr/lib/mskey/mskey-update` and a polkit policy. From this version on, the person at the computer can update MS Key without typing a password. The helper only installs a newer official `mskey` release downloaded from this repository’s GitHub Releases and checked against `SHA256SUMS`. Remote or inactive sessions still require an administrator password. Updating from 0.2.11 or earlier still asks for the password once.

### macOS installation

```sh
curl -fsSL https://github.com/buidangminh23/ms-key/releases/download/v0.2.12/install-macos.sh | bash
```

## [0.2.11] - 2026-11-22

### Verification

- This patch packages the Telex engine from v0.2.10 after a full-dictionary typing check in Codex on macOS. All 6,711 entries matched the text read back from the composer, including spaces between words. Examples included `cacs` → “các”, `hoawcj` → “hoặc” and `ddaayf` → “đầy”.
- The check used automated, overlapping keystrokes with a target interval of 7 ms in 28 batches. Expected output followed the configured modern tone placement for 69 dictionary entries. Physical keyboards were not exercised in this check.

### macOS installation

```sh
curl -fsSL https://github.com/buidangminh23/ms-key/releases/download/v0.2.11/install-macos.sh | bash
```

## [0.2.10] - 2026-11-22

### Fixed

- Telex accepts the tone key immediately after `qu`, before the next vowel. For example, `qufa` produces “quà” and `qusooc` produces “quốc”.
- Telex accepts a late `w` in “Đắk” (`DDakws`) and “Lắk” (`Lakws`) and preserves the tone in “gỵa” (`gyja`).
- Vietnamese mode keeps valid Vietnamese spellings when the same keys also spell an English word. For example, `gary` produces “gảy” and `visa` produces “vía”. Press Escape before the word boundary to restore the literal spelling, or use English mode.

### Verification

- The regression suite checks 126 distinct Telex sequences as individual words and in continuous text, including spaces, punctuation, tabs and line breaks. It also checks modern and traditional tone placement and literal English input.

## [0.2.9] - 2026-11-22

### Fixed

- On macOS, MS Key tracks the focused app and its input field outside the keyboard callback. A busy editor no longer makes each keystroke wait for a window or field lookup. Secure Input still bypasses composition.
- Restoring an English word at a word boundary keeps the original letters in names such as `Amanda` and words such as `banana`. Explicit Telex escapes for repeated letters still work.

### Verification

- Added dictionary regression tests and a macOS keyboard test covering all 29 Vietnamese letters, upper and lower case, six tones, overlapping key presses, mixed text, and a temporarily unresponsive editor.

## [0.2.8] - 2026-11-21

### Fixed

- On Windows, the updater now checks that the new MS Key process owns a settings window that responds to a message. This prevents an unnecessary rollback when a working window fails the Windows input-idle check. A hung window or a process that quits during startup still triggers recovery.
- An update started in v0.2.7 uses that version's readiness check. If it restores v0.2.7, run the Windows x64 Setup from this release once. Updates started in v0.2.8 use the new check.

## [0.2.7] - 2026-11-21

### Fixed

- On Windows, MS Key now starts its update helper with a hidden console so Windows PowerShell can prepare the installation, wait for MS Key to close and reopen the updated app.
- Windows users on v0.2.6 who see an error after confirming the download can upgrade once by running the Windows x64 Setup from this release. The fix takes effect when v0.2.7 starts the next update.

### Known issue

- On Windows, the updater can restore the previous version even after the new settings window opens and responds. Windows can report an input-idle timeout for a working MS Key window. If the updater reports “The updated app did not start its interface”, run the Windows x64 Setup directly and select “Mở MS Key ngay” on the final page.

## [0.2.6] - 2026-11-21

### Fixed

- The Windows updater prepares a backup while MS Key is still running, limits how long it waits for the installer and the new app, and attempts to reopen the previous version if installation or startup fails.
- The macOS updater limits download, signing and launch waits. On Linux, the settings window reports a slow package installation and lets users check its status while the package manager finishes.
- On Windows, delayed edits at a word boundary now finish before the first letter of the next word. Holding Shift while pressing Space also keeps the space after the accented letter.
- On Windows, typing in a newly focused text field starts a fresh word, including when an app moves focus between fields in the same window.
- Repeating a tone key to enter it literally now keeps the displayed spelling when pressing Space, including words that still contain Vietnamese diacritics. English and custom dictionary spellings are still recognized.

## [0.2.4] - 2026-11-18

### Fixed

- Standalone Vietnamese letters keep their tone when a word is committed with automatic spelling restoration enabled.
- The Vietnamese engine preserves the dictionary spellings Nguơn and palăng when entering horn and breve marks.

## [0.2.3] - 2026-11-18

### Fixed

- On Windows, rapid typing keeps subsequent text behind pending Vietnamese replacements. If Windows rejects a replacement entirely, MS Key lets the original key through.
- On macOS, MS Key sends each Unicode scalar in its own key event, preserves incoming multi-character events and clears stale composition when the event tap resumes.

## [0.2.2] - 2026-11-17

### Fixed

- **macOS updates** keep a backup until the new app confirms startup, and restore the previous version if the replacement cannot open.

### Changed

- **macOS updates** use a dedicated window with release notes, live download progress, verification, and restart status.

## [0.2.1] - 2026-11-17

### Fixed

- **macOS** now checks the active keyboard layout on the main thread, avoiding a crash when typing on macOS 27 with the non-US layout restriction enabled.
- **macOS** posts deletion, replacement text, and caret movements in order through the active event tap so surrounding keystrokes such as Space cannot overtake the replacement.

## [0.2.0] - 2026-11-17

### Added

- **Gõ thử** opens a dedicated typing workspace from the main window. Try the built-in input methods or your custom key map, switch between Vietnamese and English for the test, and clear the text without changing your saved settings. Telex, VNI and VIQR show example keystrokes.

### Changed

- **Windows, macOS and Linux settings** have a redesigned main window with a visible Vietnamese/English switch, quick access to conversion and typing practice, and related options grouped under Cơ bản, Phím tắt and Hệ thống. Mở rộng keeps advanced options and JSON backup together.
- **Windows** shows keyboard focus on custom controls and updates the settings window when it moves between monitors with different display scaling.

## [0.1.11] - 2026-11-17

### Fixed

- **`đ` in short abbreviations**: typing `hdd` gave `hdd` instead of `hđ`, and `ddc` showed `đc` but went back to `ddc` on space. In a word with no vowel (`hđ`, `đc`, `đk`, `vđ`, `tđ`) `dd` now turns the last `d` into `đ`, and the word is kept when you press space or punctuation. Typing `d` once more gives the plain letters back (`hddd` → `hdd`).
- **Tone typed right after `gi`**: `gifo` gave `gìo` instead of `giò`; the tone now moves onto the main vowel once it is typed.
- **Windows: letters landing in the wrong place when typing fast**: in Compatible Hook mode MS Key sent the backspaces, waited 2 ms, then sent the new text, so a key pressed during that wait was inserted between them. Each replacement is now sent in one batch that other key presses cannot split.
- **Windows: crash `RefCell already mutably borrowed` in `ui_paint.rs`**: redrawing the settings window while it was being updated no longer panics and restarts MS Key.

## [0.1.10] - 2026-11-17

### Removed

- **Remembering V/E per application** ("Tự động nhớ V/E theo ứng dụng"): MS Key no longer keeps a V or E choice for each app, on Windows, Linux or macOS. On macOS this memory switched the mode to English in the middle of typing: choosing V from the menu bar did not update the remembered value for the current app, so the next key press restored the old E for that app. The V/E mode now only changes when you switch it yourself or when an app matches a rule in "Danh sách ứng dụng" (Always English, Always Vietnamese, Ignore). The setting is gone from all three settings windows, the old `smart_app_switch` key in `config.json` is ignored and `app_state.json` is no longer read or written.

## [0.1.9] - 2026-11-16

### Fixed

- **macOS asking for Accessibility again after every update**: releases are ad-hoc signed, so each build had a new code hash and macOS treated it as a different app. The curl installer, the DMG installer and the in-app updater now sign MS Key with a certificate created once on the Mac (`~/Library/Application Support/MSKey/signing`, its own keychain, never added to the search list). The designated requirement becomes `identifier "org.mskey.mac" and certificate leaf = H"…"`, which stays the same across updates, so Accessibility only has to be allowed once more after this switch.

## [0.1.8] - 2026-11-16

### Fixed

- **Windows: MS Key closing by itself**: a panic in the release build aborted the app (`0xc0000409`) with nothing recorded, so the input method just disappeared. MS Key now writes the panic message and source location to `%APPDATA%\MSKey\crash.log` and starts itself again when it had been running for at least 30 seconds (a crash right at startup is not retried, to avoid a loop).

## [0.1.7] - 2026-11-16

### Fixed

- **V/E switch popup still showing after 0.1.4**: settings saved before the popup default changed kept `show_switch_osd: true`, so the popup never went away for existing users. Configs without `config_version` now get the switch popup and caret HUD turned off once (Windows, Linux, CLI and macOS); turning them back on afterwards is kept.
- **Windows tray click switching to English**: clicking or right-clicking the tray icon made the taskbar (`explorer.exe`) the foreground app, so an `explorer.exe` "always English" rule flipped the mode to E. The taskbar, tray overflow and MS Key's own windows no longer change the current app or its V/E mode; right-click only opens the quick settings menu.
- **Windows single-instance**: a second launch now opens the running instance's window, and the single-instance lock is scoped per desktop.

## [0.1.6] - 2026-11-16

### Added

- **Check-for-updates & SHA-256 verified self-update across all platforms**:
  - **Core (`mskey-core::update`) & CLI (`mskey update`)**: compares versions against GitHub's latest release, verifies release assets against `SHA256SUMS` using built-in SHA-256 verification, and supports `--check`, `--download`, `--apply`, `--target`, `--to-dir`, and `--json`.
  - **Windows (`mskey-win`)**: adds "Kiểm tra cập nhật" / auto-check support in Settings and Tray, downloads installer or portable ZIP builds into `%TEMP%\mskey-update`, verifies SHA-256, and spawns a detached PowerShell updater that cleanly relaunches MS Key (preserving elevated task start when appropriate).
  - **macOS (`MSKeyMac`)**: updates `UpdateChecker` to download, verify `SHA256SUMS`, extract/stage, and install `.zip` updates over the running `.app` bundle via Settings/menu actions.
  - **Linux (`mskey-linux`)**: adds update check/download and `.deb` / tarball self-update helpers to GTK Settings and Tray.

### Fixed

- **Telex `ư`/`w`, double-key reverts, and `Backspace` re-typing**:
  - Preserved `auto_o_horn` across trailing consonants and tone keys so accidental trailing `w` keystrokes do not break words such as `được`, `nước`, and `người`.
  - Tightened `can_insert_w_as_u_horn` prefix checks so `new`, `view`, `review`, `kw`, `pw`, `quw`, `ghw`, and `nghw` do not turn `w` into `ư` mid-typing.
  - Normalized open `ươ` syllables on delimiters (`thuwor ` → `thuở `, `huwo ` → `huơ `, `khuwo ` → `khuơ `) and validated `hoa/hoă/oă/oeo/băn khoăn` and tone-before-modifier sequences alongside checked-coda tone rules.
  - Synchronized reverted `raw` buffers (`sync_reverted_plain_ascii`) across `d`-stroke, circumflex, breve, horn, and `w` reverts and reset revert state on `Backspace` (`beee\x08e` → `bê`, `tuww\x08w` → `tư`, `ddd\x08d` → `đ`, `cooo\x08o` → `cô`, `toiss\x08s` → `tói`, `beee\x08t` → `bet`, `tuww\x08i` → `tui`).
- **English auto-restore accuracy**:
  - Added inflected stem matching (`-s`, `-es`, `-ed`, `-ing` for stems ≥ 4 ASCII chars), apostrophe contraction restoration (`didn't`, `hadn't`, `doesn't`, `wasn't`), reverted-circumflex + `đ` preservation (`ddeee ` → `đee `), and `f`/`j`/`w`/`z` onset restoration (`foos`, `jaws`), plus expanded `english_dict_merged.txt` with non-colliding tech and `Cw`/`ee`/`oo` English words (`dwarf`, `dwell`, `tweet`, etc.).
- **Windows hook stability & modifier handling**:
  - Fixed modifier key tracking (per-side Ctrl/Shift/Alt/Win + CapsLock state), async UIA password detection hardening, and `CompatibleHook` `dispatch_payload` modifier cycling so fast uppercase typing does not leave `Shift` stuck.

### Downloads

- Windows 10/11: `MSKey-Setup-0.1.6-windows-x64.exe`, `MSKey-0.1.6-windows-x64-portable.zip`, or `MSKey-0.1.6-windows-arm64-portable.zip` (plus `mskey-cli-0.1.6-windows-x64.zip`).
- macOS 13+ (Intel and Apple Silicon): run `curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash` or install from `MSKey-0.1.6-macos-universal.dmg` (if downloaded via browser, run `xattr -dr com.apple.quarantine /Applications/MSKey.app`).
- Linux (amd64 / arm64): `mskey_0.1.6_amd64.deb`, `mskey_0.1.6_arm64.deb`, `MSKey-0.1.6-linux-amd64.tar.gz`, `MSKey-0.1.6-linux-arm64.tar.gz`, and `mskey-cli-0.1.6-linux-*.tar.gz`.
- npm installer: `npx @minhspark/ms-key@0.1.6`

## [0.1.5] - 2026-11-16

### Fixed

- **Windows: MS Key no longer stops after three days of uptime.** With "Chạy với quyền Admin" on, MS Key starts at sign-in through the scheduled task `MSKeyAutoStart`. The task was created with `schtasks /TR`, which applies Task Scheduler's defaults: a 72-hour run limit and Below Normal priority, so on a PC that stays on Task Scheduler closed MS Key after three days. The task is now registered from an XML definition with no run limit, Normal priority, a 5-second delay after sign-in, three restarts one minute apart if it fails, and a trigger bound to the signed-in user's SID. Saving settings from an elevated MS Key rewrites an existing task with these values.
- **Windows: no console flash when the startup task is updated.** `schtasks` now runs without a window.

### Downloads

- Windows 10/11: `MSKey-Setup-0.1.5-windows-x64.exe`, `MSKey-0.1.5-windows-x64-portable.zip` or `MSKey-0.1.5-windows-arm64-portable.zip`. To repair an existing task, open MS Key's settings with "Chạy với quyền Admin" and "Khởi động cùng Windows" on and save once.
- macOS 13+ (Intel and Apple Silicon): run `curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash`. The DMG `MSKey-0.1.5-macos-universal.dmg` also works; after a browser download, run `xattr -dr com.apple.quarantine /Applications/MSKey.app`. macOS and Linux have no behaviour change in this release.
## [0.1.4] - 2026-11-16

### Changed

- The V/E popup no longer appears when you switch between Vietnamese and English. The mode-switch popup (`show_switch_osd`) and the caret HUD (`show_caret_osd`) are now off by default on Windows, macOS and Linux. Existing settings are kept, and both can still be turned on in settings.

### Added

- **Smart Code Bypass** (`smart_code_bypass`): automatically leaves camelCase, `snake_case`/`__init__`, alphanumeric tokens (such as `sha256`), URLs, email addresses, and file paths unaccented.
- **Hold-to-English & CapsLock Quick Switch**: temporarily hold a modifier/key (`RightShift`, `LeftShift`, `RightAlt`, `RightCtrl`, `CapsLock`) for English input, or use single-tap CapsLock quick switching (`quick_capslock_switch`).
- **Undo Last Word & Selection Converter hotkeys**: `Ctrl+Shift+Z` restores raw keystrokes on the previous committed word; `Ctrl+Shift+F7` converts selected text in-place (with `Ctrl+Shift+F6` preserved for clipboard conversion).
- **Dynamic Macros**: added `{cursor}` (`$|`), `{clipboard}`, and `{date:FORMAT}` (alongside `{date}`, `{time}`, `{datetime}`, `{year}`, `{month}`, `{day}`, `{weekday}`), plus optional Tab-only macro triggering (`macro_trigger_tab_only`).
- **Surrounding-text Re-edit & Caret HUD**: re-edit existing words before the cursor (`reedit_surrounding_word`) and show a lightweight V/E indicator near the caret (`show_caret_osd`).
- **UIA Password detection (Windows)**: queries Windows UI Automation (`IUIAutomation` `IsPassword`) asynchronously so password inputs stay bypassed automatically.
- **Window Title Rules, Profiles & Config Sync**: added `title_pattern` wildcard rules per app, built-in/custom Configuration Profiles (`Mặc định`, `Lập trình`, `Soạn thảo`), and external config synchronization (`sync_config_path`).
- **`mskey` CLI (`crates/mskey-cli`)**: added standalone command-line tool with `convert`, `detect`, `check`, `import-evkey`, and `macros export`/`import` subcommands.
- **Packaging & Release Matrix**: added manifests under `packaging/` for WinGet, Scoop, Homebrew (Cask + CLI Formula), and AUR (`PKGBUILD` + `.SRCINFO`); expanded CI with `cargo test -p mskey-cli` and `cargo clippy -- -D warnings`, and added `windows-arm64`, `linux-arm64`, and universal `mskey-cli` release builds.

### Fixed

- **macOS: typing no longer stops silently after an update.** MS Key is ad-hoc signed, so every new build gets a new code hash and macOS stops honouring the old Accessibility grant. The app used to keep showing a red **V** while converting nothing. Now the letter turns grey, the menu explains that Accessibility is missing and offers **Cấp quyền Trợ năng...** and **Mở cài đặt Trợ năng...**, and a left click opens that menu instead of switching V/E. MS Key checks every 2 seconds and starts typing again as soon as access is granted, with no restart. It also rebuilds an event tap that macOS has disabled, and it can no longer end up with two taps when the first one starts slowly. The system permission prompt appears at most once per app version.

### Downloads

- macOS 13+ (Intel and Apple Silicon): run `curl -fsSL https://github.com/buidangminh23/ms-key/releases/latest/download/install-macos.sh | bash`. The DMG `MSKey-0.1.4-macos-universal.dmg` also works; after a browser download, run `xattr -dr com.apple.quarantine /Applications/MSKey.app`. After updating, macOS may stop honouring the old Accessibility grant: remove MS Key from System Settings → Privacy & Security → Accessibility with "–" and allow it again.
- Windows 10/11: `MSKey-Setup-0.1.4-windows-x64.exe`, `MSKey-0.1.4-windows-x64-portable.zip` or `MSKey-0.1.4-windows-arm64-portable.zip`.
- Linux: `mskey_0.1.4_amd64.deb`, `mskey_0.1.4_arm64.deb`, `MSKey-0.1.4-linux-amd64.tar.gz` or `MSKey-0.1.4-linux-arm64.tar.gz`.
- Command-line tool `mskey`: `mskey-cli-0.1.4-<platform>.tar.gz` or `.zip` for Windows, Linux and macOS.

All downloads include SHA-256 checksums.

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

[Unreleased]: https://github.com/buidangminh23/ms-key/compare/v0.1.3...HEAD
[0.1.0]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.0
[0.1.1]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.1
[0.1.2]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.2
[0.1.3]: https://github.com/buidangminh23/ms-key/releases/tag/v0.1.3
