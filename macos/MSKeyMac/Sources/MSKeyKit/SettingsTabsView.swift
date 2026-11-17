import AppKit
import SwiftUI

public struct SettingsTabsView: View {
    @ObservedObject public var store: MSKeyStore
    @ObservedObject private var updater = UpdateChecker.shared
    public let selectedTab: Int
    @State private var profileNameInput = ""

    public init(store: MSKeyStore, selectedTab: Int) {
        self.store = store
        self.selectedTab = selectedTab
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                switch selectedTab {
                case 0: basicTab
                case 1: hotkeyTab
                default: systemTab
                }
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
        }
        .id(selectedTab)
    }

    private var basicTab: some View {
        Group {
            SettingsSection("Dấu & chính tả") {
                HStack(alignment: .top, spacing: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        option("Đặt dấu kiểu mới (oà, uý)", \.modern_tone)
                        option("Đặt dấu tự do", \.free_tone)
                        option("Cho phép f, j, w, z", \.allow_fjwz)
                        option("Viết hoa đầu câu", \.auto_capitalize_sentence)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    VStack(alignment: .leading, spacing: 8) {
                        option("Kiểm tra chính tả", \.spelling_check)
                        option("Khôi phục phím từ sai", \.auto_restore_wrong_word)
                        option("Bỏ qua từ kỹ thuật / code", \.smartCodeBypass)
                        option("Sửa tiếp từ trước con trỏ", \.reeditSurroundingWord)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            SettingsSection("Tương thích ứng dụng") {
                HStack(alignment: .top, spacing: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        option("Dùng clipboard gửi phím", \.use_clipboard_send)
                        option("Sử dụng SendKey chuẩn", \.standard_key_send)
                        option("Tắt khi không phải phím US", \.disable_on_non_us_layout)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    VStack(alignment: .leading, spacing: 8) {
                        option("Sửa lỗi dấu của Gợi ý 1", \.fix_browser_suggest_1)
                        option("Sửa lỗi dấu của Gợi ý 2", \.fix_browser_suggest_2)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            SettingsSection("Gõ tắt & phím nhanh") {
                HStack(spacing: 8) {
                    option("Cho phép gõ tắt", \.macro_enabled)
                    Spacer(minLength: 8)
                    Button("Bảng gõ tắt…") { store.openSheet(.macros) }
                    Button("Mở rộng & sao lưu…") { store.openSheet(.extras) }
                }
                option("Vẫn gõ tắt khi tắt Tiếng Việt", \.macro_when_english)
                option("Chỉ kích hoạt gõ tắt bằng Tab", \.macroTriggerTabOnly)
                Divider()
                HStack(spacing: 16) {
                    option("Gõ w thành ư", \.quick_w_to_u_horn)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    option("Gõ [ thành ơ, ] thành ư", \.quick_brackets)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

        }
    }

    private var hotkeyTab: some View {
        Group {
            SettingsSection("Chuyển Tiếng Việt / Tiếng Anh") {
                HStack(spacing: 16) {
                    Picker("Phím chuyển V/E", selection: binding(\.switch_preset)) {
                        Text("Ctrl + Shift").tag(SwitchPresetKind.ctrlShift)
                        Text("Alt / Option + Z").tag(SwitchPresetKind.altZ)
                        Text("Ctrl + Space").tag(SwitchPresetKind.ctrlSpace)
                        Text("Shift + Shift").tag(SwitchPresetKind.shiftShift)
                        Text("Phím tùy chọn").tag(SwitchPresetKind.custom)
                    }
                    Picker("Giữ để gõ Tiếng Anh", selection: binding(\.holdEnglishKey)) {
                        ForEach(HoldEnglishKey.allCases) { key in
                            Text(key.label).tag(key)
                        }
                    }
                }
                hotkeyRow("Tổ hợp tùy chọn", combo: store.config.custom_switch_hotkey, sheet: .captureSwitchHotkey)
                Divider()
                option("Bấm nhanh Caps Lock để chuyển V/E", \.quickCapslockSwitch)
                option("Phát âm thanh khi chuyển V/E", \.beep_on_switch)
                HStack(spacing: 16) {
                    option("Hiện chỉ báo nổi khi chuyển V/E", \.show_switch_osd)
                    option("Thu gọn chỉ báo cạnh con trỏ", \.showCaretOsd)
                        .disabled(!store.config.show_switch_osd)
                }
            }

            SettingsSection("Phím tắt công cụ") {
                hotkeyRow("Chuyển mã clipboard", combo: store.config.clipboard_convert_hotkey, sheet: .captureConvertHotkey)
                Divider()
                hotkeyRow("Chuyển đổi vùng chọn", combo: store.config.convertSelectionHotkey, sheet: .captureConvertSelectionHotkey)
                Divider()
                hotkeyRow("Hoàn tác từ vừa gõ", combo: store.config.undoLastWordHotkey, sheet: .captureUndoHotkey)
                Divider()
                option("Phím Esc khôi phục từ gốc", \.esc_restore)
            }

            SettingsSection("Cấu hình EVKey") {
                HStack(spacing: 8) {
                    Button("Nhập EVKey setting.tree…") { store.importEVKeyTreePanel() }
                    Button("Mở thư mục cấu hình") { NSWorkspace.shared.open(MacConfigStore.configDirectory) }
                }
            }
        }
    }

    private var systemTab: some View {
        Group {
            if !store.accessibilityGranted {
                accessibilityNotice
            }

            SettingsSection("Khởi động & giao diện") {
                option("Khởi động cùng macOS", \.auto_start)
                option("Hiện bảng điều khiển khi mở", \.show_on_startup)
                Divider()
                Picker("Giao diện", selection: binding(\.theme)) {
                    ForEach(ThemeModeKind.allCases) { theme in
                        Text(theme.label).tag(theme)
                    }
                }
                .frame(maxWidth: 240)
            }

            SettingsSection("Hồ sơ cấu hình") {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Đang dùng")
                            .foregroundStyle(.secondary)
                        Text(store.config.activeProfile)
                            .fontWeight(.medium)
                            .lineLimit(1)
                            .help(store.config.activeProfile)
                    }
                    Spacer(minLength: 8)
                    Button("Xoá hồ sơ") { store.deleteCustomProfile(named: store.config.activeProfile) }
                        .disabled(MSKeyConfig.builtinProfileNames.contains(store.config.activeProfile))
                }
                Divider()
                HStack(spacing: 8) {
                    TextField("Tên hồ sơ mới hoặc cập nhật", text: $profileNameInput)
                    Button("Lưu hồ sơ") { saveProfile() }
                        .disabled(profileName.isEmpty)
                }
            }

            SettingsSection("Quy tắc V/E theo ứng dụng") {
                let activeApp = store.lastActiveApp.isEmpty ? "Chưa xác định" : store.lastActiveApp
                Text("Ứng dụng gần nhất: \(activeApp)")
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .help(activeApp)
                HStack(spacing: 8) {
                    Button("Danh sách ứng dụng…") { store.openSheet(.appRules) }
                    Button("Thêm ứng dụng vào Tiếng Anh") { store.addLastAppForceEnglish() }
                        .disabled(store.lastActiveApp.isEmpty)
                }
            }

            SettingsSection("Đồng bộ & cập nhật") {
                HStack(spacing: 8) {
                    Text("Tệp đồng bộ")
                    let path = store.config.syncConfigPath
                    Text(path.isEmpty ? "Chưa thiết lập" : path)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .help(path)
                    Spacer(minLength: 8)
                    Button("Chọn…") { store.chooseSyncConfigPath() }
                    Button("Tắt đồng bộ") { store.clearSyncConfigPath() }
                        .disabled(path.isEmpty)
                }
                Divider()
                HStack(spacing: 8) {
                    option("Tự kiểm tra cập nhật khi mở", \.checkUpdatesOnStart)
                    Spacer(minLength: 8)
                    if updater.isBusy {
                        ProgressView().controlSize(.small)
                    }
                    Button("Cập nhật MS Key…") { store.checkForUpdates(manual: true) }
                }
                if !updater.statusText.isEmpty {
                    Text(updater.statusText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            SettingsSection("Từ điển") {
                Button("Từ điển cá nhân…") { store.openSheet(.userDict) }
            }
        }
    }

    private var accessibilityNotice: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text("MS Key cần quyền Trợ năng")
                    .fontWeight(.semibold)
                Text("Cấp quyền để gõ tiếng Việt trong các ứng dụng. Nếu MS Key đã có trong danh sách mà vẫn gõ không được, hãy xoá mục đó rồi cấp lại.")
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 8)
            Button("Cấp quyền") { store.requestAccessibility() }
        }
        .padding(12)
        .background(.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))
    }

    private var profileName: String { profileNameInput.trimmingCharacters(in: .whitespacesAndNewlines) }

    private func saveProfile() {
        let name = profileName
        guard !name.isEmpty else { return }
        store.saveCurrentAsProfile(named: name)
        profileNameInput = ""
    }

    private func hotkeyRow(_ title: String, combo: HotkeyComboModel, sheet: ActiveSheetKind) -> some View {
        HStack(spacing: 8) {
            Text(title)
            Spacer(minLength: 8)
            Text(combo.displayText)
                .font(.caption.monospaced())
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
            Button("Đổi phím") { store.openSheet(sheet) }
                .accessibilityLabel("Đổi phím: \(title)")
        }
        .frame(minHeight: 24)
    }

    private func option(_ title: String, _ keyPath: WritableKeyPath<MSKeyConfig, Bool>) -> some View {
        Toggle(title, isOn: binding(keyPath))
            .toggleStyle(.checkbox)
    }

    private func binding<Value>(_ keyPath: WritableKeyPath<MSKeyConfig, Value>) -> Binding<Value> {
        Binding(
            get: { store.config[keyPath: keyPath] },
            set: { value in store.updateConfig { $0[keyPath: keyPath] = value } }
        )
    }
}
