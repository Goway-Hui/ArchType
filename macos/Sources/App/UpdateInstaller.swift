// 更新 ArchType（macOS）：查 GitHub 上的最新正式版，下载 ArchType-<版本>-macos.pkg 和同一页的 SHA256SUMS、校验，
// 交给系统的 installer 只装给当前用户（-target CurrentUserHomeDirectory，不要密码，和 install.sh 一样）。
// 规矩（包名、比版本、多久查一次）在 Shared/AppUpdate.swift。
//
// 只在设置窗口进程里下载、安装，输入法进程只查有没有新版（AppState.checkUpdateIfDue）。
// pkg 的 preinstall 会停掉所有 ArchType 进程（设置窗口也是 ArchType），所以 installer 由一个脱离出去的 sh 跑：
// 装完把退出码写进数据目录的 update/result，删掉 pkg；设置窗口被停掉了就重新打开它，新窗口读 result 显示结果。
// postinstall 照常登记、打开、切到 ArchType。

import Foundation

enum UpdateInstaller {
    /// 上次查的时间和那时最新的 tag（AppUpdate.State）：输入法和设置窗口共用
    static func stateFile(in supportDir: URL) -> URL {
        supportDir.appendingPathComponent("update-state")
    }

    /// 下载的安装包、installer 的输出和结果放这里
    static func folder(in supportDir: URL) -> URL {
        supportDir.appendingPathComponent("update", isDirectory: true)
    }

    static func resultFile(in supportDir: URL) -> URL {
        folder(in: supportDir).appendingPathComponent("result")
    }

    static func logFile(in supportDir: URL) -> URL {
        folder(in: supportDir).appendingPathComponent("installer.log")
    }

    static var releasesPage: URL {
        URL(string: AppUpdate.releasesPage)!
    }

    /// 这个版本的发布页；tag 拼不成网址时是总的发布页
    static func releasePage(tag: String) -> URL {
        URL(string: AppUpdate.releasesPage + "/tag/" + tag) ?? releasesPage
    }

    static func readState(supportDir: URL) -> AppUpdate.State {
        AppUpdate.State(text: readText(stateFile(in: supportDir).path) ?? "")
    }

    static func writeState(_ state: AppUpdate.State, supportDir: URL) {
        try? writeText(state.text, to: stateFile(in: supportDir).path)
    }

    /// 问 GitHub 最新正式版（beta：Beta 计划开着，预发布也算），查到了记进 update-state（输入法菜单、下一次检查用）。
    /// 会等网络，在后台线程上调。
    static func fetchLatest(supportDir: URL, beta: Bool = false) throws -> LexiconPackInstaller.Release {
        let release = try beta ? LexiconPackInstaller.fetchNewest() : LexiconPackInstaller.fetchLatest()
        guard AppUpdate.version(tag: release.tag) != nil else {
            throw PackError("发布页上的版本号看不懂：\(release.tag)")
        }
        writeState(AppUpdate.State(checked: Int64(Date().timeIntervalSince1970), latest: release.tag),
                   supportDir: supportDir)
        return release
    }

    /// 下载这个版本的 macOS 安装包，按同一页的 SHA256SUMS 校验，返回下好的 pkg（数据目录的 update/ 里；
    /// 断了再点接着下）。校验不过删掉、抛错。在后台线程上调。
    static func download(_ release: LexiconPackInstaller.Release, supportDir: URL, control: DownloadControl,
                         progress: @escaping (Int64, Int64) -> Void) throws -> URL {
        guard let version = AppUpdate.version(tag: release.tag) else {
            throw PackError("发布页上的版本号看不懂：\(release.tag)")
        }
        let name = AppUpdate.packageName(version: version)
        guard let pkgURL = release.assets[name] else {
            throw PackError("\(release.tag) 的发布页上没有 macOS 安装包 \(name)")
        }
        guard let sumsURL = release.assets[LexiconPack.checksumFile] else {
            throw PackError("发布页上缺少 \(LexiconPack.checksumFile)，不安装")
        }
        let sums = String(decoding: try LexiconPackInstaller.get(sumsURL, timeout: 20), as: UTF8.self)
        let expected: String
        switch LexiconPack.checksums(sums, wanted: [name]) {
        case .success(let found): expected = found[name] ?? ""
        case .failure(let error): throw error
        }
        let dest = folder(in: supportDir).appendingPathComponent(name)
        // 上次下好了没装上的那份不用（重新校验一遍下载的；.part 还在的接着下）
        try? FileManager.default.removeItem(at: dest)
        try LocalModelInstaller.download(pkgURL.absoluteString, sha256: expected, to: dest, control: control,
                                         progress: progress)
        return dest
    }

    /// 交给系统的 installer 装，马上返回（装完由脱离出去的 sh 写 update/result）。
    static func launchInstaller(pkg: URL, supportDir: URL) throws {
        let fm = FileManager.default
        try fm.createDirectory(at: folder(in: supportDir), withIntermediateDirectories: true)
        let result = resultFile(in: supportDir)
        try? fm.removeItem(at: result)
        // 路径都当参数传（$1…$5），不拼进脚本。watcher 是这个设置窗口的进程号：装完它还在就由它自己读结果，
        // 被 preinstall 停掉了就重新打开设置窗口（装好的新版在 ~/Library/Input Methods）
        let script = """
        pkg=$1 log=$2 result=$3 app=$4 watcher=$5
        /usr/sbin/installer -pkg "$pkg" -target CurrentUserHomeDirectory >"$log" 2>&1
        code=$?
        /bin/rm -f "$pkg"
        printf '%s\\n' "$code" >"$result.tmp" && /bin/mv -f "$result.tmp" "$result"
        /bin/sleep 1
        if ! /bin/kill -0 "$watcher" 2>/dev/null; then
          new="$HOME/Library/Input Methods/ArchType.app"
          if [ -d "$new" ]; then app=$new; fi
          /usr/bin/open -n "$app" --args --settings general
        fi
        """
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", script, "archtype-update", pkg.path, logFile(in: supportDir).path, result.path,
                             Bundle.main.bundleURL.path, String(ProcessInfo.processInfo.processIdentifier)]
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
    }

    /// 装完的结果：update/result 在就读出来并删掉；还没装完、没在装是 nil。
    static func takeResult(supportDir: URL) -> (ok: Bool, detail: String)? {
        let path = resultFile(in: supportDir).path
        guard let text = readText(path) else { return nil }
        try? FileManager.default.removeItem(atPath: path)
        let code = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if code == "0" {
            return (true, "")
        }
        // installer 失败时最后一行是原因（installer: The install failed …）
        let log = readText(logFile(in: supportDir).path) ?? ""
        let last = log.split(whereSeparator: \.isNewline).last.map(String.init) ?? ""
        return (false, "installer 退出码 \(code)" + (last.isEmpty ? "" : "，\(last)"))
    }
}
