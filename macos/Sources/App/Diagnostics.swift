// 诊断信息（设置窗口「帮助」→「复制诊断信息」）：版本、输入法状态、设置文件（打码）、数据文件在不在和大小、
// 词库和各个模型的状态、最近的错误、系统日志里 ArchType 最近的记录。内容照 Linux 设置的 collect_diagnostics：
// 不含打过的字、自定义词和输入习惯的内容（只有条数），家目录写成 ~。

import AppKit
import AVFoundation
import CArchtype
import Foundation

/// 读文本文件；不在是 nil。不是 UTF-8 也读出来（坏的字节换成 �），免得当成没有文件、整个重写。
/// 12.3 MB 这样的写法。（原来在删掉的 SettingsModel.swift 里，App 各文件都还在用。）
func readText(_ path: String) -> String? {
    guard let data = FileManager.default.contents(atPath: path) else { return nil }
    return String(decoding: data, as: UTF8.self)
}

/// 整个换掉（先写临时文件再改名）。文件是个链接（比如链到同步盘）时写链接指向的文件。
func writeText(_ text: String, to path: String) throws {
    let url = URL(fileURLWithPath: path).resolvingSymlinksInPath()
    try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try text.write(to: url, atomically: true, encoding: .utf8)
}

enum Diagnostics {
    /// 在后台线程上调（读文件、跑 log show 要几秒）。inputSource 是 Installer.sourceSummary（TIS 要在主线程上问），
    /// errors 是设置窗口里各处最近的错误。
    static func collect(inputSource: String, errors: [String]) -> String {
        let state = AppState.shared
        let fm = FileManager.default
        let support = state.supportDir
        let configText = readText(state.configPath)
        let config = ArchtypeConfig.parse(configText ?? "")
        var out: [String] = []
        func add(_ line: String) { out.append(line) }

        add("# ArchType 诊断信息（macOS）")
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss Z"
        add("导出时间：\(formatter.string(from: Date()))")
        add("不含你打过的字、自定义词和输入习惯的内容（只有条数和大小）。")
        add("")

        add("## 版本")
        add("ArchType：\(appVersion)，核心 \(String(cString: archtype_version()))，接口 \(Engine.abiVersion)")
        add("系统：macOS \(ProcessInfo.processInfo.operatingSystemVersionString)，\(LocalModelInstaller.machine)")
        add("程序：\(Bundle.main.bundleURL.path)")
        if let data = state.resource("DATA-VERSION").flatMap(readText) {
            add("安装包里的词库：\(data.trimmingCharacters(in: .whitespacesAndNewlines))")
        }
        add("")

        add("## 输入法")
        add("输入源：\(inputSource)")
        let me = ProcessInfo.processInfo.processIdentifier
        let running = NSRunningApplication.runningApplications(withBundleIdentifier: Installer.bundleID)
            .filter { $0.processIdentifier != me && !$0.isTerminated }
        let processes = running.map { app -> String in
            let pid = String(app.processIdentifier)
            return app.activationPolicy == .regular ? pid + "（设置窗口）" : pid
        }
        add("别的 ArchType 进程：" + (processes.isEmpty ? "没有（输入法没在运行）" : processes.joined(separator: "、")))
        add("")

        add("## 设置（archtype.conf）")
        if let configText {
            out.append(DiagnosticsText.maskConfig(configText))
        } else {
            add("（没有设置文件，都是默认值）")
        }
        add("互译 Key：" + (TranslateKeyFile.read(state.translateKeyPath).isEmpty ? "没设置" : "已设置"))
        add("")

        add("## 数据文件")
        let wanxiang = LexiconPackInstaller.lexicon(in: support)
        let configured = (config.lexicon as NSString).expandingTildeInPath
        if config.lexicon.isEmpty {
            add("  词库：雾凇（自带）")
        } else {
            let kind = configured == wanxiang ? "万象" : "自己指定"
            let missing = fm.fileExists(atPath: configured) ? "" : "（文件不在，用的是自带的）"
            add("  词库：\(kind)，\(configured)\(missing)")
        }
        add("  万象词库包：\(LexiconPack.installedVersion(folder: LexiconPackInstaller.folder(in: support).path) ?? "没装")")
        let resources = ["lexicon.tsv", "english.txt", "words.lm", "s2t.tsv", "wubi86.tsv", "nnlm.bin"]
            .map { state.resourceDir.appendingPathComponent($0).path }
        let data = ["lexicon.tsv", "lexicon.tsv.wcache", "words.domain.lm", "wetype/\(NnlmInstaller.fileName)",
                    LexiconPack.folderName, LexiconPack.folderName + "/lexicon.tsv.wcache",
                    "wtlocal", "models/\(LocalModel.fileName)", "llama", "update-state"]
            .map { support.appendingPathComponent($0).path }
        for path in resources + data {
            add(fileLine(path))
        }
        for (label, path) in [("自定义词", state.customWordsPath), ("自动学到的词", state.userDictPath),
                              ("输入习惯", state.historyPath)] {
            add("  \(label)：" + (countLines(path).map { "\($0) 条" } ?? "没有"))
        }
        add("")

        add("## 模型")
        if let nnlm = state.nnlmPath() {
            let vocab = NnlmInstaller.check(nnlm)
            add("整句增强模型：\(nnlm)，" + (vocab.map { "字表 \($0)" } ?? "解析不了"))
        } else {
            add("整句增强模型：没有")
        }
        let voiceModel = VoiceModelInstaller.isReady(supportDir: support) ? "已下载" : "没下载"
        let voiceSwitch = config.voice ? "开（\(config.voiceKey)）" : "关"
        add("语音模型：\(voiceModel)，语音输入\(voiceSwitch)")
        add("麦克风：" + microphone())
        let model = config.translateLocalModel.isEmpty
            ? state.localModelPath : (config.translateLocalModel as NSString).expandingTildeInPath
        let plan = LocalModelInstaller.plan(model: model, customServer: config.translateLlamaServer, supportDir: support)
        let service = "翻译服务 \(DiagnosticsText.maskURL(config.translateBaseUrl)) \(config.translateModel)"
        add("互译：\(config.translate ? "开" : "关")，\(config.translateLocal ? "本地模型" : service)")
        var lacking: [String] = []
        if plan.wantModel { lacking.append("模型") }
        if plan.wantLlama { lacking.append(plan.llamaOutdated ? "llama-server（下载的构建旧了）" : "llama-server") }
        if plan.llamaUnavailable { lacking.append("llama-server（这台 Mac 下载不了）") }
        add("本地翻译模型：" + (plan.ready ? "已就绪" : "缺" + lacking.joined(separator: "、")))
        if let server = LocalModelInstaller.findServer(custom: config.translateLlamaServer, supportDir: support) {
            add("llama-server：\(server)" + (LocalModelInstaller.downloadedBuild(in: support).map { "（下载的 \($0)）" } ?? ""))
        }
        let update = UpdateInstaller.readState(supportDir: support)
        var checked = "还没查过"
        if update.checked > 0 {
            let when = formatter.string(from: Date(timeIntervalSince1970: TimeInterval(update.checked)))
            checked = "上次 \(when)，最新 \(update.latest)"
        }
        add("检查更新：\(config.checkUpdate ? "开" : "关")，\(checked)")
        add("")

        add("## 最近的错误")
        out.append(contentsOf: errors.isEmpty ? ["（设置窗口里没有）"] : errors)
        if let log = readText(UpdateInstaller.logFile(in: support).path), !log.isEmpty {
            add("上次更新时 installer 的输出（最后 20 行）：")
            out.append(contentsOf: log.split(whereSeparator: \.isNewline).suffix(20).map(String.init))
        }
        add("")

        add("## 系统日志（ArchType 进程，最近 2 小时，最后 300 行）")
        let log = (try? LocalModelInstaller.run("/usr/bin/log", ["show", "--last", "2h", "--style", "compact",
                                                                 "--predicate", "process == \"ArchType\""], timeout: 60))
            ?? ""
        let lines = log.split(whereSeparator: \.isNewline).suffix(300).map(String.init)
        out.append(contentsOf: lines.isEmpty ? ["（读不到）"] : lines)

        var text = out.joined(separator: "\n") + "\n"
        let home = NSHomeDirectory()
        if !home.isEmpty && home != "/" {
            text = text.replacingOccurrences(of: home, with: "~")
        }
        return text
    }

    /// 「  路径：12.3 MB，2026-11-17 10:00」；目录算里面所有文件；不在写「没有」
    private static func fileLine(_ path: String) -> String {
        let fm = FileManager.default
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: path, isDirectory: &isDir) else { return "  \(path)：没有" }
        let size = isDir.boolValue ? LexiconPackInstaller.folderSize(URL(fileURLWithPath: path))
            : LocalModelInstaller.fileSize(URL(fileURLWithPath: path).resolvingSymlinksInPath().path)
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        let modified = (try? fm.attributesOfItem(atPath: path))?[.modificationDate] as? Date
        let megabytes = String(format: "%.1f", Double(size) / 1_000_000)
        return "  \(path)：\(megabytes) MB" + (modified.map { "，\(formatter.string(from: $0))" } ?? "")
    }

    /// 不算注释、空行的行数；文件不在是 nil
    private static func countLines(_ path: String) -> Int? {
        guard let text = readText(path) else { return nil }
        return text.split(whereSeparator: \.isNewline).filter { line in
            let body = line.drop(while: { $0 == " " || $0 == "\t" })
            return !body.isEmpty && !body.hasPrefix("#")
        }.count
    }

    private static func microphone() -> String {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return "已允许"
        case .notDetermined: return "还没问过"
        case .restricted: return "被系统限制"
        default: return "没允许"
        }
    }
}
