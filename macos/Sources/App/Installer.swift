// 向系统登记输入法（install.sh 装好以后调用）。和 Squirrel 的做法一样：
// TISRegisterInputSource 登记安装包，TISEnableInputSource 打开，TISSelectInputSource 切过去。
// 卸载（--uninstall、设置窗口「帮助」→「卸载 ArchType」）也在这里。

import AppKit
import Carbon
import Foundation

enum Installer {
    static let bundleID = Bundle.main.bundleIdentifier ?? "io.github.gloridust.inputmethod.ArchType"
    static var modeID: String { bundleID + ".Hans" }

    static func register() -> Bool {
        let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
        print(status == noErr ? "已登记 \(Bundle.main.bundleURL.path)" : "登记失败（\(status)）")
        return status == noErr
    }

    /// 这个安装包登记过的输入源（includeAllInstalled：没打开的也要列出来）。
    private static func sources() -> [TISInputSource] {
        let filter = [kTISPropertyBundleID as String: bundleID] as CFDictionary
        guard let list = TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource] else {
            return []
        }
        return list
    }

    private static func string(_ source: TISInputSource, _ key: CFString) -> String? {
        guard let raw = TISGetInputSourceProperty(source, key) else { return nil }
        return Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
    }

    private static func bool(_ source: TISInputSource, _ key: CFString) -> Bool {
        guard let raw = TISGetInputSourceProperty(source, key) else { return false }
        return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(raw).takeUnretainedValue())
    }

    private static func mode() -> TISInputSource? {
        sources().first { string($0, kTISPropertyInputSourceID) == modeID }
    }

    static func enable() -> Bool {
        guard let source = mode() else {
            print("没找到输入源 \(modeID)：先 --register，可能要等几秒或重新登录")
            return false
        }
        if bool(source, kTISPropertyInputSourceIsEnabled) {
            print("已经打开了")
            return true
        }
        let status = TISEnableInputSource(source)
        print(status == noErr ? "已打开 \(modeID)" : "打开失败（\(status)）")
        return status == noErr
    }

    static func select() -> Bool {
        guard let source = mode() else {
            print("没找到输入源 \(modeID)")
            return false
        }
        let status = TISSelectInputSource(source)
        print(status == noErr ? "已切到 ArchType" : "切换失败（\(status)）")
        return status == noErr
    }

    static func disable() -> Bool {
        var ok = true
        for source in sources() where bool(source, kTISPropertyInputSourceIsEnabled) {
            ok = TISDisableInputSource(source) == noErr && ok
        }
        print(ok ? "已关闭" : "关闭失败")
        return ok
    }

    /// 系统现在用的输入源是不是 ArchType（InitialMode 用来分清切走了输入法还是只换了应用）；问不到是 nil。在主线程上调。
    static var isCurrentSource: Bool? {
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue() else { return nil }
        if let bundle = string(source, kTISPropertyBundleID) {
            return bundle == bundleID
        }
        return string(source, kTISPropertyInputSourceID).map { $0.hasPrefix(bundleID) }
    }

    /// 诊断信息用：系统里登记了没有、打开了没有、是不是正在用（在主线程上调）
    static var sourceSummary: String {
        guard let source = mode() else { return "系统里没有登记（没找到 \(modeID)）" }
        let enabled = bool(source, kTISPropertyInputSourceIsEnabled)
        let selected = bool(source, kTISPropertyInputSourceIsSelected)
        return "已登记，" + (enabled ? "已打开" : "没打开") + (selected ? "，正在用" : "")
    }

    // MARK: 卸载

    /// 和 lsregister -u 一样从 LaunchServices 去掉这个程序的登记
    private static let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/"
        + "LaunchServices.framework/Support/lsregister"

    /// 装好的 ArchType.app：现在跑的这个（在「Input Methods」目录里才算，构建目录里的不删），
    /// 和 ~/Library/Input Methods 里的那个
    static func installedApps() -> [URL] {
        var apps: [URL] = []
        let me = Bundle.main.bundleURL.standardizedFileURL
        if me.lastPathComponent == "ArchType.app", me.deletingLastPathComponent().lastPathComponent == "Input Methods" {
            apps.append(me)
        }
        let user = URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Input Methods/ArchType.app")
            .standardizedFileURL
        if FileManager.default.fileExists(atPath: user.path), !apps.contains(user) {
            apps.append(user)
        }
        return apps
    }

    /// 卸载：关掉输入源、停掉别的 ArchType 进程（输入法收到 SIGTERM 会先存学习数据）、从 LaunchServices
    /// 去掉登记、删掉程序。用户数据（~/Library/Application Support/ArchType）不动。返回 (都做到了, 给人看的说明)。
    static func uninstall(supportDir: URL) -> (ok: Bool, message: String) {
        let apps = installedApps()
        guard !apps.isEmpty else {
            return (false, "没找到装好的 ArchType.app（~/Library/Input Methods 里没有）")
        }
        _ = disable()
        // 别的 ArchType 进程：输入法（系统按需启动）、另一个设置窗口
        let me = ProcessInfo.processInfo.processIdentifier
        let others = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            .filter { $0.processIdentifier != me && !$0.isTerminated }
        for app in others {
            kill(app.processIdentifier, SIGTERM)
        }
        let deadline = Date().addingTimeInterval(3)
        while others.contains(where: { !$0.isTerminated }) && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        }
        for app in others where !app.isTerminated {
            app.forceTerminate()
        }
        var problems: [String] = []
        for app in apps {
            _ = try? LocalModelInstaller.run(lsregister, ["-u", app.path], timeout: 30)
            do {
                try FileManager.default.removeItem(at: app)
            } catch {
                problems.append("删不掉 \((app.path as NSString).abbreviatingWithTildeInPath)：\(error.localizedDescription)")
            }
        }
        // 菜单栏的输入法菜单会缓存输入法列表和图标，重启一下它（系统会马上把它拉起来）
        _ = try? LocalModelInstaller.run("/usr/bin/killall", ["TextInputMenuAgent"], timeout: 10)
        let data = (supportDir.path as NSString).abbreviatingWithTildeInPath
        if !problems.isEmpty {
            return (false, problems.joined(separator: "\n") + "\n可以在访达里手动删掉。用户数据还在 \(data)。")
        }
        return (true, "已卸载 ArchType。自定义词、学到的词和设置还在 \(data)，不要了可以手动删掉。"
                + "输入法列表里还看得到 ArchType 的话，注销重新登录一次就没了。")
    }
}
