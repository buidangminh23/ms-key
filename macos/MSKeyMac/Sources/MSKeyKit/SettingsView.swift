import SwiftUI

public struct SettingsView: View {
    @ObservedObject public var store: MSKeyStore
    @State private var selectedTab = 0
    @State private var brandVisible = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init(store: MSKeyStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(nsImage: MSKeyBrand.mark)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 48, height: 40)
                    .scaleEffect(brandVisible ? 1 : 0.8)
                    .opacity(brandVisible ? 1 : 0.25)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("MS Key").font(.title3.bold())
                    Text("Gõ tiếng Việt theo cách của riêng bạn")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text("v\(MSKeyBrand.version)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            headerSection
            Picker("", selection: $selectedTab) {
                Text("Cơ bản").tag(0)
                Text("Phím tắt").tag(1)
                Text("Hệ thống").tag(2)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            SettingsTabsView(store: store, selectedTab: selectedTab)
                .background(.background.opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.secondary.opacity(0.2), lineWidth: 1))

            footerSection
        }
        .padding(12)
        .frame(width: 560, height: 624)
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
            case .extras:
                ExtrasSandboxView(store: store)
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

    private var headerSection: some View {
        HStack(alignment: .top, spacing: 12) {
            GroupBox("Điều khiển") {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 8) {
                    GridRow {
                        Text("Bảng mã").frame(width: 68, alignment: .leading)
                        Picker("", selection: binding(\.charset)) {
                            ForEach(CharsetKind.allCases) { cs in
                                Text(cs.label).tag(cs)
                            }
                        }
                        .labelsHidden()
                        .gridCellColumns(2)

                        Button(action: { store.toggleVE() }) {
                            Text(store.effectiveVE ? "Tiếng Việt (V)" : "Tiếng Anh (E)")
                                .fontWeight(.semibold)
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(store.effectiveVE ? .red : .blue)
                        .frame(width: 132, height: 28)
                    }

                    GridRow {
                        Text("Kiểu gõ").frame(width: 68, alignment: .leading)
                        Picker("", selection: binding(\.input_method)) {
                            ForEach(InputMethodKind.allCases) { m in
                                Text(m.label).tag(m)
                            }
                        }
                        .labelsHidden()

                        Button("...") {
                            store.openSheet(.customKeymap)
                        }
                        .frame(width: 36, height: 28)

                        Button("Chuyển mã...") {
                            store.openSheet(.converter)
                        }
                        .frame(width: 132, height: 28)
                    }

                    GridRow {
                        Text("Cơ chế").frame(width: 68, alignment: .leading)
                        Picker("", selection: binding(\.mechanism)) {
                            ForEach(MechanismKind.allCases) { mech in
                                Text(mech.label).tag(mech)
                            }
                        }
                        .labelsHidden()
                        .gridCellColumns(2)

                        Text("App: \(store.lastActiveApp.isEmpty ? "-" : store.lastActiveApp)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .frame(width: 132, alignment: .leading)
                    }
                }
                .padding(8)
            }

            VStack(spacing: 8) {
                Button("Đóng") {
                    store.closeMainWindow()
                }
                .frame(width: 84, height: 32)

                Button("Thoát") {
                    NSApp.terminate(nil)
                }
                .frame(width: 84, height: 32)
            }
            .padding(.top, 8)
        }
    }

    private var footerSection: some View {
        HStack(spacing: 8) {
            Button("Th.Lập mặc định") {
                store.resetDefaults()
            }
            .frame(height: 32)

            Button("Lưu") {
                store.saveNow()
            }
            .buttonStyle(.borderedProminent)
            .frame(height: 32)

            Spacer()

            Text("MS Key • Tiếng Việt Unicode (macOS)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
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

    private func binding<T>(_ keyPath: WritableKeyPath<MSKeyConfig, T>) -> Binding<T> {
        Binding(
            get: { store.config[keyPath: keyPath] },
            set: { newVal in
                store.updateConfig { $0[keyPath: keyPath] = newVal }
            }
        )
    }
}
