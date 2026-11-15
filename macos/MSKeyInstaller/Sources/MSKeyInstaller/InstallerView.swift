import AppKit
import SwiftUI

struct InstallerView: View {
    @ObservedObject var model: InstallerModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let pink = Color(red: 0.95, green: 0.18, blue: 0.38)
    private let blue = Color(red: 0.02, green: 0.43, blue: 0.98)

    private var mark: NSImage {
        Bundle.main.url(forResource: "mskey-mark", withExtension: "png")
            .flatMap(NSImage.init(contentsOf:)) ?? NSApp.applicationIconImage
    }

    var body: some View {
        VStack(spacing: 22) {
            VStack(spacing: 6) {
                Text(model.installedApp == nil ? "Cài đặt MS Key" : "Cài đặt hoàn tất")
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                Text("Bộ gõ tiếng Việt · \(model.version)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            transferView
            VStack(spacing: 10) {
                HStack {
                    Text(model.status).font(.headline)
                    Spacer()
                    if model.isInstalling && model.phase == .copying && model.totalBytes > 0 {
                        Text("\(Int(min(1, Double(model.completedBytes) / Double(model.totalBytes)) * 100))%")
                            .monospacedDigit().foregroundStyle(.secondary)
                    }
                }
                if model.installedApp != nil {
                    ProgressView(value: 1).tint(.green)
                } else if model.isInstalling && model.phase == .copying && model.totalBytes > 0 {
                    ProgressView(value: Double(model.completedBytes), total: Double(model.totalBytes)).tint(blue)
                    Text(model.byteDescription).font(.caption).monospacedDigit().foregroundStyle(.secondary)
                } else if model.isInstalling {
                    if reduceMotion { Text("Vui lòng đợi…").font(.caption).foregroundStyle(.secondary) }
                    else { ProgressView().controlSize(.small).frame(height: 18) }
                } else {
                    ProgressView(value: 0).tint(blue)
                }
            }
            HStack(spacing: 10) {
                Image(systemName: "folder").foregroundStyle(blue)
                VStack(alignment: .leading, spacing: 3) {
                    Text("Thư mục cài đặt").font(.caption).foregroundStyle(.secondary)
                    Text(model.destination.path).font(.callout).lineLimit(1).truncationMode(.middle)
                        .help(model.destination.path)
                }
                Spacer(minLength: 8)
                if !model.isInstalling && model.installedApp == nil {
                    Button("Thay đổi…", action: model.chooseDestination)
                }
            }
            .padding(14)
            .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 12))
            if let error = model.errorMessage {
                Text(error).font(.callout).foregroundStyle(.red)
                    .textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            } else {
                Text(model.installedApp == nil
                     ? "Thoát MS Key đang chạy trước khi cài. Cấu hình gõ hiện có được giữ nguyên."
                     : "Mở MS Key và cấp quyền Trợ năng trong Cài đặt hệ thống. Sau đó có thể tháo ổ đĩa cài đặt.")
                    .font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                if model.installedApp != nil {
                    Button("Hiện trong Finder", action: model.revealInstalledApp)
                    Spacer()
                    Button("Mở MS Key", action: model.openInstalledApp).buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                } else if model.isInstalling {
                    Spacer()
                    Button(model.isCancelling ? "Đang hủy…" : "Hủy", action: model.cancel)
                        .disabled(model.isCancelling || model.phase == .finishing)
                } else {
                    Button("Đóng") { NSApp.terminate(nil) }
                    Spacer()
                    Button(model.errorMessage == nil ? "Cài đặt" : "Thử lại", action: model.install)
                        .buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
                }
            }
        }
        .padding(32)
        .frame(width: 570)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var transferView: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30, paused: !model.isInstalling || reduceMotion)) { context in
            let time = context.date.timeIntervalSinceReferenceDate
            let moving = model.isInstalling && !reduceMotion
            HStack(spacing: 18) {
                Image(nsImage: mark).resizable().scaledToFit()
                    .frame(width: 92, height: 92)
                    .scaleEffect(moving ? 1 + 0.035 * sin(time * 3) : 1)
                    .frame(width: 118, height: 118)
                    .background(.background, in: RoundedRectangle(cornerRadius: 26))
                    .overlay(RoundedRectangle(cornerRadius: 26).stroke(pink.opacity(0.16)))
                    .shadow(color: pink.opacity(0.10), radius: 18, y: 8)
                HStack(spacing: 8) {
                    ForEach(0..<4) { index in
                        Circle().fill(LinearGradient(colors: [pink, blue], startPoint: .leading, endPoint: .trailing))
                            .frame(width: 7, height: 7)
                            .opacity(moving ? 0.25 + 0.75 * ((sin(time * 5 - Double(index) * 0.9) + 1) / 2) : 0.35)
                    }
                    Image(systemName: "chevron.right").font(.system(size: 15, weight: .semibold)).foregroundStyle(blue)
                }
                ZStack(alignment: .bottomTrailing) {
                    Image(nsImage: NSWorkspace.shared.icon(forFile: "/Applications"))
                        .resizable().scaledToFit().frame(width: 96, height: 96)
                    if model.installedApp != nil {
                        Image(systemName: "checkmark.circle.fill").font(.system(size: 27))
                            .foregroundStyle(.white, .green)
                    }
                }.frame(width: 118, height: 118)
            }.frame(maxWidth: .infinity).padding(.vertical, 4)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Cài MS Key vào Applications")
    }
}
