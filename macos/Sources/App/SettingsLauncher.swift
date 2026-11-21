// 设置界面是 NSH 写的跨平台 app（bundle id io.github.gowayhui.archtype.settings），
// 不再内置 SwiftUI 设置窗口。这里只负责找到它，并用 --page 打开对应页。
import AppKit

enum SettingsPage: String {
    case general
    case words
    case voice
}

enum SettingsLauncher {
    static let bundleID = "io.github.gowayhui.archtype.settings"

    static func open(page: SettingsPage? = nil) {
        var candidates: [URL] = []
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            candidates.append(url)
        }
        if let path = ProcessInfo.processInfo.environment["ARCHTYPE_SETTINGS_APP"], !path.isEmpty {
            candidates.append(URL(fileURLWithPath: path))
        }
        // 安装包里自带的设置 app（ArchType.app/Contents/Resources/ArchType 设置.app）
        if let builtin = Bundle.main.resourceURL?.appendingPathComponent("ArchType 设置.app"),
           FileManager.default.fileExists(atPath: builtin.path) {
            candidates.append(builtin)
        }
        for name in ["ArchType 设置", "ArchType Settings"] {
            if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: name) {
                candidates.append(url)
            }
        }

        guard let url = candidates.first else {
            NSLog("ArchType: 找不到设置 app（\(bundleID)），请先安装「ArchType 设置」")
            return
        }
        let config = NSWorkspace.OpenConfiguration()
        config.activates = true
        if let page {
            config.arguments = ["--page", page.rawValue]
        }
        NSWorkspace.shared.openApplication(at: url, configuration: config) { _, error in
            if let error {
                NSLog("ArchType: 打开设置失败：\(error.localizedDescription)")
            }
        }
    }
}
