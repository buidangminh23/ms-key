import SwiftUI

public struct SettingsView: View {
    static let defaultSize = NSSize(width: 704, height: 676)
    static let minimumSize = NSSize(width: 640, height: 480)

    @ObservedObject public var store: MSKeyStore
    @State private var selectedTab = 0
    @State private var brandVisible = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var modeFocused: Bool

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 16) {
                brandHeader
                inputControls
                navigation
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
            .background(Color(nsColor: .windowBackgroundColor))

            Divider()
            SettingsTabsView(store: store, selectedTab: selectedTab)
                .frame(maxHeight: .infinity)
            Divider()
            footer
        }
        .frame(minWidth: Self.minimumSize.width, minHeight: Self.minimumSize.height)
        .background(Color(nsColor: .windowBackgroundColor))
        .task(id: store.presentationID) { await animateBrand() }
        .sheet(item: $store.activeSheet) { sheet in
            switch sheet {
            case .macros:
                MacroManagerView(store: store)
            case .customKeymap:
                CustomKeymapView(store: store)
            case .appRules:
                AppRulesView(store: store)
            case .converter:
                ConverterView(store: store)
            case .sandbox:
                TypingSandboxView(store: store)
            case .extras:
                AdvancedSettingsView(store: store)
            case .userDict:
                UserDictView(store: store)
            case .captureSwitchHotkey:
                HotkeyCaptureView(store: store, target: .switchHotkey)
            case .captureConvertHotkey:
                HotkeyCaptureView(store: store, target: .convertHotkey)
            case .captureUndoHotkey:
                HotkeyCaptureView(store: store, target: .undoHotkey)
            case .captureConvertSelectionHotkey:
                HotkeyCaptureView(store: store, target: .convertSelectionHotkey)
            }
        }
    }

    private var brandHeader: some View {
        HStack(spacing: 12) {
            Image(nsImage: MSKeyBrand.mark)
                .resizable()
                .scaledToFit()
                .frame(width: 48, height: 48)
                .scaleEffect(brandVisible ? 1 : 0.8)
                .opacity(brandVisible ? 1 : 0.25)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text("MS Key")
                    .font(.system(size: 26, weight: .semibold))
                    .accessibilityAddTraits(.isHeader)
                Text("Bộ gõ tiếng Việt")
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 8)
            if !store.accessibilityGranted {
                Button("Cấp quyền…") { store.requestAccessibility() }
                    .help("Cấp quyền Trợ năng để gõ tiếng Việt trong ứng dụng khác")
            }
            inputActions
            Button {
                store.toggleVE()
            } label: {
                Text(store.effectiveVE ? "Tiếng Việt (V)" : "Tiếng Anh (E)")
                    .fontWeight(.semibold)
                    .foregroundStyle(.white)
                    .frame(minWidth: 104, minHeight: 32)
                    .padding(.horizontal, 8)
                    .background(modeColor, in: RoundedRectangle(cornerRadius: 8))
                    .contentShape(RoundedRectangle(cornerRadius: 8))
            }
            .buttonStyle(.plain)
            .focused($modeFocused)
            .overlay {
                if modeFocused {
                    RoundedRectangle(cornerRadius: 10)
                        .strokeBorder(Color(nsColor: .keyboardFocusIndicatorColor), lineWidth: 2)
                        .padding(-4)
                        .allowsHitTesting(false)
                }
            }
            .help("Chuyển giữa Tiếng Việt và Tiếng Anh")
            .accessibilityHint("Chuyển giữa Tiếng Việt và Tiếng Anh")
        }
    }

    private var modeColor: Color {
        store.effectiveVE
            ? Color(red: 0.72, green: 0.15, blue: 0.20)
            : Color(red: 0, green: 0.36, blue: 0.63)
    }

    private var inputControls: some View {
        HStack(alignment: .bottom, spacing: 12) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Kiểu gõ").font(.caption.weight(.semibold))
                HStack(spacing: 8) {
                    Picker("Kiểu gõ", selection: binding(\.input_method)) {
                        ForEach(InputMethodKind.allCases) { method in
                            Text(method.label).tag(method)
                        }
                    }
                    .labelsHidden()
                    Button {
                        store.openSheet(.customKeymap)
                    } label: {
                        Image(systemName: "slider.horizontal.3")
                            .frame(minWidth: 20, minHeight: 16)
                    }
                    .help("Tự định nghĩa kiểu gõ")
                    .accessibilityLabel("Tự định nghĩa kiểu gõ")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            VStack(alignment: .leading, spacing: 8) {
                Text("Bảng mã").font(.caption.weight(.semibold))
                Picker("Bảng mã", selection: binding(\.charset)) {
                    ForEach(CharsetKind.allCases) { charset in
                        Text(charset.label).tag(charset)
                    }
                }
                .labelsHidden()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            VStack(alignment: .leading, spacing: 8) {
                Text("Cơ chế").font(.caption.weight(.semibold))
                Picker("Cơ chế", selection: binding(\.mechanism)) {
                    ForEach(MechanismKind.allCases) { mechanism in
                        Text(mechanism.label).tag(mechanism)
                    }
                }
                .labelsHidden()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .controlSize(.regular)
        .settingsCard()
    }

    private var inputActions: some View {
        HStack(spacing: 8) {
            Button("Gõ thử…") { store.openSheet(.sandbox) }
            Button("Chuyển mã…") { store.openSheet(.converter) }
        }
        .fixedSize()
    }

    private var navigation: some View {
        HStack(spacing: 16) {
            Picker("Mục cài đặt", selection: $selectedTab) {
                Text("Cơ bản").tag(0)
                Text("Phím tắt").tag(1)
                Text("Hệ thống").tag(2)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 304)

            Spacer(minLength: 8)

            Picker("Hồ sơ", selection: Binding(
                get: { store.config.activeProfile },
                set: { _ = store.applyProfile($0) }
            )) {
                ForEach(store.config.allProfileNames(), id: \.self) { name in
                    Text(name).tag(name)
                }
            }
            .frame(maxWidth: 228)
        }
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Text("v\(MSKeyBrand.version)")
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            Button("Thoát MS Key") { NSApp.terminate(nil) }
                .buttonStyle(.borderless)
                .padding(.leading, 8)
            Spacer(minLength: 16)
            Button("Khôi phục mặc định") { store.resetDefaults() }
            Button("Đóng") { store.closeMainWindow() }
            Button("Lưu cài đặt") { store.saveNow() }
                .buttonStyle(.borderedProminent)
        }
        .controlSize(.large)
        .padding(.horizontal, 24)
        .padding(.vertical, 12)
    }

    private func animateBrand() async {
        guard !reduceMotion && !CommandLine.arguments.contains("--verify-smoke") else {
            brandVisible = true
            return
        }
        brandVisible = false
        do {
            try await Task.sleep(nanoseconds: 20_000_000)
        } catch {
            return
        }
        withAnimation(.easeOut(duration: 0.45)) {
            brandVisible = true
        }
    }

    private func binding<Value>(_ keyPath: WritableKeyPath<MSKeyConfig, Value>) -> Binding<Value> {
        Binding(
            get: { store.config[keyPath: keyPath] },
            set: { value in store.updateConfig { $0[keyPath: keyPath] = value } }
        )
    }
}

struct SettingsSection<Content: View>: View {
    let title: LocalizedStringKey
    let content: Content

    init(_ title: LocalizedStringKey, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.subheadline.weight(.semibold))
                .accessibilityAddTraits(.isHeader)

            VStack(alignment: .leading, spacing: 8) {
                content
            }
            .settingsCard()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

extension View {
    func settingsCard() -> some View {
        self
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background {
                RoundedRectangle(cornerRadius: 12).fill(Color(nsColor: .controlBackgroundColor))
            }
            .overlay {
                RoundedRectangle(cornerRadius: 12).strokeBorder(.secondary.opacity(0.24))
                    .allowsHitTesting(false)
            }
    }

    func settingsSheet(width: CGFloat) -> some View {
        self
            .padding(24)
            .frame(width: width, alignment: .leading)
            .background(Color(nsColor: .windowBackgroundColor))
    }
}

struct SettingsSheetTitle: View {
    let title: LocalizedStringKey
    var detail: LocalizedStringKey? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.title3.weight(.semibold))
                .accessibilityAddTraits(.isHeader)
            if let detail {
                Text(detail).foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
