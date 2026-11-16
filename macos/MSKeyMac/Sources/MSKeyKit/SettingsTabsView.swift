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
        switch selectedTab {
        case 0: basicTab
        case 1: hotkeyTab
        default: systemTab
        }
    }

    private var basicTab: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top, spacing: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Đặt dấu kiểu mới (oà, uý)", isOn: binding(\.modern_tone))
                        Toggle("Đặt dấu tự do", isOn: binding(\.free_tone))
                        Toggle("Cho phép f, j, w, z", isOn: binding(\.allow_fjwz))
                        Toggle("Viết hoa đầu câu", isOn: binding(\.auto_capitalize_sentence))
                        Toggle("Dùng clipboard gửi phím", isOn: binding(\.use_clipboard_send))
                        Toggle("Sử dụng SendKey chuẩn", isOn: binding(\.standard_key_send))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Kiểm tra chính tả", isOn: binding(\.spelling_check))
                        Toggle("Khôi phục phím từ sai", isOn: binding(\.auto_restore_wrong_word))
                        Toggle("Bỏ qua thông minh khi gõ code", isOn: binding(\.smartCodeBypass))
                        Toggle("Sửa tiếp từ trước con trỏ", isOn: binding(\.reeditSurroundingWord))
                        Toggle("Tắt khi không phải phím US", isOn: binding(\.disable_on_non_us_layout))
                        Toggle("Sửa lỗi dấu của Gợi ý 1", isOn: binding(\.fix_browser_suggest_1))
                        Toggle("Sửa lỗi dấu của Gợi ý 2", isOn: binding(\.fix_browser_suggest_2))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }

                GroupBox("Gõ tắt & phím tắt nhanh") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Toggle("Cho phép gõ tắt", isOn: binding(\.macro_enabled))
                            Spacer()
                            Button("Bảng gõ tắt...") {
                                store.openSheet(.macros)
                            }
                            .frame(height: 28)
                        }
                        Toggle("Vẫn gõ tắt khi tắt Tiếng Việt", isOn: binding(\.macro_when_english))
                        Toggle("Chỉ bung gõ tắt bằng phím Tab", isOn: binding(\.macroTriggerTabOnly))
                        HStack(spacing: 16) {
                            Toggle("Gõ w thành ư", isOn: binding(\.quick_w_to_u_horn))
                            Toggle("Gõ [ thành ơ, ] thành ư", isOn: binding(\.quick_brackets))
                        }
                    }
                    .padding(8)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
        }
    }

    private var hotkeyTab: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                GroupBox("Phím chuyển V/E") {
                    VStack(alignment: .leading, spacing: 8) {
                        Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                            GridRow {
                                presetOption("Ctrl + Shift", .ctrlShift)
                                presetOption("Alt / Option + Z", .altZ)
                                presetOption("Ctrl + Space", .ctrlSpace)
                            }
                            GridRow {
                                presetOption("Shift + Shift", .shiftShift)
                                presetOption("Phím tùy chọn", .custom)
                                Color.clear.frame(width: 1, height: 1)
                            }
                        }

                        HStack(spacing: 8) {
                            Text("Tổ hợp tùy chọn: \(store.config.custom_switch_hotkey.displayText)")
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("Đặt phím tùy chọn") {
                                store.openSheet(.captureSwitchHotkey)
                            }
                            .frame(height: 28)
                        }

                        HStack(spacing: 8) {
                            Text("Giữ để gõ Tiếng Anh:")
                            Picker("", selection: binding(\.holdEnglishKey)) {
                                ForEach(HoldEnglishKey.allCases) { item in
                                    Text(item.label).tag(item)
                                }
                            }
                            .labelsHidden()
                            .frame(width: 170)
                            Spacer()
                        }

                        Toggle("Bấm Caps Lock chuyển nhanh V/E", isOn: binding(\.quickCapslockSwitch))
                    }
                    .padding(8)
                }

                Toggle("Phím Esc khôi phục từ gốc", isOn: binding(\.esc_restore))
                Toggle("Âm thanh báo khi chuyển V/E", isOn: binding(\.beep_on_switch))
                HStack(spacing: 16) {
                    Toggle("Hiện OSD nổi khi chuyển V/E", isOn: binding(\.show_switch_osd))
                    Toggle("OSD gọn cạnh con trỏ", isOn: binding(\.showCaretOsd))
                        .disabled(!store.config.show_switch_osd)
                }

                GroupBox("Phím tắt chức năng & Nhập EVKey") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Text("Hoàn tác từ vừa gõ: \(store.config.undoLastWordHotkey.displayText)")
                                .font(.subheadline)
                            Spacer()
                            Button("Đổi phím") {
                                store.openSheet(.captureUndoHotkey)
                            }
                            .frame(height: 26)
                        }
                        HStack(spacing: 8) {
                            Text("Chuyển đổi vùng chọn: \(store.config.convertSelectionHotkey.displayText)")
                                .font(.subheadline)
                            Spacer()
                            Button("Đổi phím") {
                                store.openSheet(.captureConvertSelectionHotkey)
                            }
                            .frame(height: 26)
                        }
                        HStack(spacing: 8) {
                            Text("Chuyển mã Clipboard: \(store.config.clipboard_convert_hotkey.displayText)")
                                .font(.subheadline)
                            Spacer()
                            Button("Đổi phím chuyển mã") {
                                store.openSheet(.captureConvertHotkey)
                            }
                            .frame(height: 26)
                        }
                        HStack(spacing: 8) {
                            Button("Nhập EVKey setting.tree...") {
                                store.importEVKeyTreePanel()
                            }
                            .frame(height: 28)
                            Button("Mở thư mục cấu hình") {
                                NSWorkspace.shared.open(MacConfigStore.configDirectory)
                            }
                            .frame(height: 28)
                            Spacer()
                        }
                    }
                    .padding(8)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
        }
    }

    private var systemTab: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if !store.accessibilityGranted {
                    HStack(spacing: 8) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                        Text("Cần cấp quyền Trợ năng (Accessibility) để gõ Tiếng Việt. Sau khi cập nhật MS Key, macOS có thể không nhận quyền cũ: nếu MS Key đã có trong danh sách mà vẫn không gõ được, xoá nó (–) rồi cấp lại.")
                            .font(.caption)
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer()
                        Button("Cấp quyền") {
                            store.requestAccessibility()
                        }
                        .frame(height: 24)
                    }
                    .padding(8)
                    .background(Color.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                }

                Toggle("Khởi động cùng macOS", isOn: binding(\.auto_start))
                Toggle("Hiện bảng điều khiển khi mở", isOn: binding(\.show_on_startup))

                HStack(spacing: 8) {
                    Text("Giao diện")
                        .frame(width: 80, alignment: .leading)
                    Picker("", selection: binding(\.theme)) {
                        ForEach(ThemeModeKind.allCases) { t in
                            Text(t.label).tag(t)
                        }
                    }
                    .labelsHidden()
                    .frame(maxWidth: .infinity)
                }

                GroupBox("Hồ sơ cấu hình (Profiles)") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Picker("Đang dùng", selection: Binding(
                                get: { store.config.activeProfile },
                                set: { _ = store.applyProfile($0) }
                            )) {
                                ForEach(store.config.allProfileNames(), id: \.self) { name in
                                    Text(name).tag(name)
                                }
                            }
                            Button("Xoá") {
                                store.deleteCustomProfile(named: store.config.activeProfile)
                            }
                            .disabled(MSKeyConfig.builtinProfileNames.contains(store.config.activeProfile))
                            .frame(height: 26)
                        }
                        HStack(spacing: 8) {
                            TextField("Tên hồ sơ mới hoặc cập nhật...", text: $profileNameInput)
                            Button("Lưu hồ sơ") {
                                let name = profileNameInput.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard !name.isEmpty else { return }
                                store.saveCurrentAsProfile(named: name)
                                profileNameInput = ""
                            }
                            .disabled(profileNameInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            .frame(height: 26)
                        }
                    }
                    .padding(8)
                }

                GroupBox("Tự động chuyển V/E theo ứng dụng") {
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Tự động nhớ V/E theo ứng dụng", isOn: binding(\.smart_app_switch))
                        Text("Ứng dụng gần nhất: \(store.lastActiveApp.isEmpty ? "-" : store.lastActiveApp)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        HStack(spacing: 8) {
                            Button("Danh sách ứng dụng...") {
                                store.openSheet(.appRules)
                            }
                            .frame(height: 28)
                            Button("Thêm app hiện tại vào Tiếng Anh") {
                                store.addLastAppForceEnglish()
                            }
                            .disabled(store.lastActiveApp.isEmpty)
                            .frame(height: 28)
                        }
                    }
                    .padding(8)
                }

                GroupBox("Đồng bộ & Cập nhật") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Text("Đồng bộ:")
                            Text(store.config.syncConfigPath.isEmpty ? "Chưa thiết lập" : store.config.syncConfigPath)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            Button("Chọn...") {
                                store.chooseSyncConfigPath()
                            }
                            .frame(height: 26)
                            Button("Tắt") {
                                store.clearSyncConfigPath()
                            }
                            .disabled(store.config.syncConfigPath.isEmpty)
                            .frame(height: 26)
                        }
                        HStack(spacing: 8) {
                            Toggle("Tự kiểm tra cập nhật khi mở", isOn: binding(\.checkUpdatesOnStart))
                            Spacer()
                            Button(updater.isDownloading ? "Đang tải..." : (updater.isChecking ? "Đang kiểm tra..." : "Kiểm tra cập nhật...")) {
                                store.checkForUpdates(manual: true)
                            }
                            .disabled(updater.isChecking || updater.isDownloading)
                            .frame(height: 28)
                        }
                        if !updater.statusText.isEmpty {
                            Text(updater.statusText)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(8)
                }

                HStack(spacing: 8) {
                    Button("Mở rộng & Gõ thử...") {
                        store.openSheet(.extras)
                    }
                    .frame(height: 28)
                    Button("Từ điển cá nhân...") {
                        store.openSheet(.userDict)
                    }
                    .frame(height: 28)
                    Spacer()
                }
                Spacer(minLength: 0)
            }
            .padding(12)
        }
    }

    private func presetOption(_ title: String, _ preset: SwitchPresetKind) -> some View {
        Button {
            store.updateConfig { $0.switch_preset = preset }
        } label: {
            HStack(spacing: 6) {
                Image(systemName: store.config.switch_preset == preset ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(store.config.switch_preset == preset ? Color.accentColor : Color.secondary)
                Text(title)
                    .foregroundStyle(.primary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func binding<T>(_ keyPath: WritableKeyPath<MSKeyConfig, T>) -> Binding<T> {
        Binding(
            get: { store.config[keyPath: keyPath] },
            set: { newVal in
                store.updateConfig { $0[keyPath: keyPath] = newVal }
            }
        )
    }
}
