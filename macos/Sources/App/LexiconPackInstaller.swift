// 下载、安装可选的万象词库包（和 Linux 设置的「万象拼音」一样）。规矩在 Shared/LexiconPack.swift。
//
// 下载：问 GitHub 最新正式版的文件列表，取 archtype-data-wanxiang-<版本>.tar.gz，按同一页的 SHA256SUMS 校验。
// 安装：先用系统的 tar 列出包里的条目，只许四个普通文件；解到旁边的临时目录，检查好 lexicon.tsv 再换进去，
// 旧目录连同它的词库快照（lexicon.tsv.wcache）一起删掉，输入法下次载入时重建快照。

import CryptoKit
import Foundation

enum LexiconPackInstaller {
    static let releaseAPI = URL(string: "https://api.github.com/repos/Gloridust/ArchType/releases/latest")!
    /// Beta 计划：最近的几十个发布（含预发布）
    static let releasesAPI = URL(string: "https://api.github.com/repos/Gloridust/ArchType/releases?per_page=30")!

    struct Release {
        let tag: String
        let assets: [String: URL]
        var prerelease = false
        /// 发布说明（Markdown），「常规」页有新版时显示里面「这一版」那一节
        var notes = ""

        var wanxiang: (name: String, url: URL, version: String)? {
            LexiconPack.pickAsset(Array(assets.keys)).flatMap { pick in
                assets[pick.name].map { (pick.name, $0, pick.version) }
            }
        }
    }

    /// 装万象的目录：~/Library/Application Support/ArchType/dicts/wanxiang
    static func folder(in supportDir: URL) -> URL {
        supportDir.appendingPathComponent(LexiconPack.folderName, isDirectory: true)
    }

    static func lexicon(in supportDir: URL) -> String {
        folder(in: supportDir).appendingPathComponent("lexicon.tsv").path
    }

    // MARK: 下载

    /// 最新正式版。先走网页（fromWeb，不算接口次数）；网页那条路走不通时再问接口
    static func fetchLatest() throws -> Release {
        do {
            return try fromWeb(beta: false)
        } catch let webError {
            guard let release = try? fetchLatestAPI() else { throw webError }
            return release
        }
    }

    /// Beta 计划：最近的发布里版本号最大的（预发布也算）。先走网页，走不通再问接口
    static func fetchNewest() throws -> Release {
        do {
            return try fromWeb(beta: true)
        } catch let webError {
            guard let release = try? fetchNewestAPI() else { throw webError }
            return release
        }
    }

    static func fetchLatestAPI() throws -> Release {
        let data = try get(releaseAPI, timeout: 20)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw PackError("发布页的内容看不懂")
        }
        return release(json)
    }

    /// 接口的发布列表里版本号最大的（见 AppUpdate.pickNewest）；一个都挑不出来时退回最新正式版
    static func fetchNewestAPI() throws -> Release {
        let data = try get(releasesAPI, timeout: 20)
        guard let list = try JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
            throw PackError("发布页的内容看不懂")
        }
        let listed = list.map {
            AppUpdate.Listed(tag: $0["tag_name"] as? String ?? "", prerelease: $0["prerelease"] as? Bool ?? false,
                             draft: $0["draft"] as? Bool ?? false)
        }
        guard let i = AppUpdate.pickNewest(listed) else { return try fetchLatestAPI() }
        return release(list[i])
    }

    /// 不走接口拼出 Release（见 AppUpdate.webCandidates）：文件列表从这一版的 SHA256SUMS 读，下载地址按发布页的规则拼。
    /// releases.atom 里只推了 tag、还没发出来的版本没有 SHA256SUMS（404），跳过。
    /// 更新说明只在比现在装的新时才取（只有那时显示），见 notes(tag:)
    static func fromWeb(beta: Bool) throws -> Release {
        let (_, final) = try fetch(URL(string: AppUpdate.latestPage)!, timeout: 20)
        let latest = final.flatMap { AppUpdate.tag(fromLatestURL: $0.absoluteString) }
        let feed = beta ? String(decoding: try get(URL(string: AppUpdate.atomFeed)!, timeout: 20), as: UTF8.self) : nil
        for tag in AppUpdate.webCandidates(latest: latest, feed: feed) {
            guard let sumsURL = URL(string: AppUpdate.downloadURL(tag: tag, file: LexiconPack.checksumFile)) else { continue }
            let sums: Data
            do {
                sums = try get(sumsURL, timeout: 20)
            } catch let error as HTTPStatus where error.code == 404 && tag != latest {
                continue
            }
            var assets: [String: URL] = [LexiconPack.checksumFile: sumsURL]
            for name in AppUpdate.assetNames(sums: String(decoding: sums, as: UTF8.self)) {
                if let url = URL(string: AppUpdate.downloadURL(tag: tag, file: name)) { assets[name] = url }
            }
            let newer = AppUpdate.version(tag: tag).map { AppUpdate.isNewer($0, than: appVersion) } ?? false
            return Release(tag: tag, assets: assets, prerelease: tag != latest, notes: newer ? notes(tag: tag) : "")
        }
        throw PackError("GitHub 上找不到发布的版本")
    }

    /// 这一版的更新说明：先下发布页上的 RELEASE-NOTES.md（不算接口次数）；更早的版本没有这个文件，问一次接口；
    /// 都拿不到时一句「见发布页」。拿不到说明不算检查失败
    static func notes(tag: String) -> String {
        let fallback = "没取到更新说明，见发布页：\(AppUpdate.tagPage(tag))"
        guard let fileURL = URL(string: AppUpdate.downloadURL(tag: tag, file: AppUpdate.notesFile)) else { return fallback }
        do {
            return String(decoding: try get(fileURL, timeout: 20), as: UTF8.self)
        } catch let error as HTTPStatus where error.code == 404 {
            // 更早的版本：问一次接口
        } catch {
            return fallback
        }
        guard let apiURL = URL(string: AppUpdate.tagAPI(tag)),
              let data = try? get(apiURL, timeout: 20),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return fallback }
        return json["body"] as? String ?? ""
    }

    /// GitHub 一个发布的 JSON -> Release
    static func release(_ json: [String: Any]) -> Release {
        var assets: [String: URL] = [:]
        for asset in json["assets"] as? [[String: Any]] ?? [] {
            if let name = asset["name"] as? String, let link = asset["browser_download_url"] as? String,
               let url = URL(string: link) {
                assets[name] = url
            }
        }
        return Release(tag: json["tag_name"] as? String ?? "", assets: assets,
                       prerelease: json["prerelease"] as? Bool ?? false, notes: json["body"] as? String ?? "")
    }

    /// 下载最新发布页上的万象词库包并按 SHA256SUMS 校验，返回临时文件（用完删掉）。
    static func download(_ release: Release, progress: @escaping (Int64, Int64) -> Void) throws -> URL {
        guard let pack = release.wanxiang else {
            throw PackError("\(release.tag) 的发布页上还没有万象词库包")
        }
        guard let sumsURL = release.assets[LexiconPack.checksumFile] else {
            throw PackError("发布页缺少 \(LexiconPack.checksumFile)，暂不安装")
        }
        let sumsText = String(decoding: try get(sumsURL, timeout: 20), as: UTF8.self)
        let expected: String
        switch LexiconPack.checksums(sumsText, wanted: [pack.name]) {
        case .success(let sums): expected = sums[pack.name]!
        case .failure(let error): throw error
        }
        let file = try DownloadTask(url: pack.url, progress: progress).run()
        let digest = try sha256(of: file)
        guard digest == expected else {
            try? FileManager.default.removeItem(at: file)
            throw PackError("\(pack.name) 校验和不对，可能没下完整，请重试")
        }
        return file
    }

    /// 服务器回的不是 2xx。403 / 429 是接口限流（rateLimited）
    struct HTTPStatus: Error, CustomStringConvertible {
        let code: Int
        var rateLimited: Bool { code == 403 || code == 429 }
        var description: String {
            rateLimited ? "GitHub 返回 \(code)（访问次数到了上限，过一会儿再试）" : "GitHub 返回 \(code)"
        }
    }

    /// GET 一个小文件（发布页的 JSON、SHA256SUMS）；UpdateInstaller 也用
    static func get(_ url: URL, timeout: TimeInterval) throws -> Data {
        try fetch(url, timeout: timeout).data
    }

    /// GET，连同跟着跳转以后最后的地址（releases/latest 跳到哪个 tag 页）
    static func fetch(_ url: URL, timeout: TimeInterval) throws -> (data: Data, url: URL?) {
        var request = URLRequest(url: url, timeoutInterval: timeout)
        request.setValue(url.host == "api.github.com" ? "application/vnd.github+json" : "*/*",
                         forHTTPHeaderField: "Accept")
        request.setValue("ArchType", forHTTPHeaderField: "User-Agent")
        let done = DispatchSemaphore(value: 0)
        var result: Result<(data: Data, url: URL?), Error> = .failure(PackError("连不上 GitHub"))
        URLSession.shared.dataTask(with: request) { data, response, error in
            if let error {
                result = .failure(error)
            } else if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                result = .failure(HTTPStatus(code: http.statusCode))
            } else {
                result = .success((data ?? Data(), response?.url))
            }
            done.signal()
        }.resume()
        done.wait()
        return try result.get()
    }

    static func sha256(of file: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: file)
        defer { try? handle.close() }
        var hasher = SHA256()
        while let chunk = try handle.read(upToCount: 1 << 20), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    // MARK: 安装

    /// 把词库包解到 supportDir/dicts/wanxiang，返回版本。出错时原来装的不动。
    @discardableResult
    static func install(_ tarball: URL, supportDir: URL, minimum: Int = LexiconPack.minEntries) throws -> String {
        let fm = FileManager.default
        let target = folder(in: supportDir)
        try fm.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)

        // 先看清包里有什么：只许四个普通文件。名字用 -t 列的（原样），类型看 -tv 那一行的第一个字符（- 是普通文件）
        let raws = try tar(["-tzf", tarball.path]).split(separator: "\n").map(String.init)
        let types = try tar(["-tvzf", tarball.path]).split(separator: "\n").map { $0.first ?? "?" }
        guard raws.count == types.count else {
            throw PackError("词库包解不开：列出来的条目对不上")
        }
        var names: [String] = []
        for (raw, type) in zip(raws, types) {
            guard let name = LexiconPack.entryName(raw) else {
                throw PackError("词库包里有不认识的文件 \(raw)，不安装")
            }
            if name.isEmpty { continue }
            guard type == "-" else {
                throw PackError("词库包里有不认识的文件 \(raw)，不安装")
            }
            names.append(raw)
        }

        let staging = target.appendingPathExtension("new")
        try? fm.removeItem(at: staging)
        try fm.createDirectory(at: staging, withIntermediateDirectories: true)
        do {
            if !names.isEmpty {
                _ = try tar(["-xzf", tarball.path, "-C", staging.path] + names)
            }
            switch LexiconPack.checkLexicon(at: staging.appendingPathComponent("lexicon.tsv").path, minimum: minimum) {
            case .success: break
            case .failure(let error): throw error
            }
            let old = target.appendingPathExtension("old")
            try? fm.removeItem(at: old)
            if fm.fileExists(atPath: target.path) {
                try fm.moveItem(at: target, to: old)
            }
            try fm.moveItem(at: staging, to: target)
            try? fm.removeItem(at: old)
        } catch {
            try? fm.removeItem(at: staging)
            throw error
        }
        return LexiconPack.installedVersion(folder: target.path) ?? "?"
    }

    /// 删掉万象（和它的词库快照），返回空出的字节数。
    @discardableResult
    static func remove(supportDir: URL) -> Int64 {
        let target = folder(in: supportDir)
        let size = folderSize(target)
        try? FileManager.default.removeItem(at: target)
        return size
    }

    static func folderSize(_ url: URL) -> Int64 {
        let fm = FileManager.default
        guard let items = fm.enumerator(at: url, includingPropertiesForKeys: [.fileSizeKey]) else { return 0 }
        var total: Int64 = 0
        for case let item as URL in items {
            total += Int64((try? item.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
        }
        return total
    }

    private static func tar(_ args: [String]) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
        process.arguments = args
        let out = Pipe(), err = Pipe()
        process.standardOutput = out
        process.standardError = err
        try process.run()
        let data = out.fileHandleForReading.readDataToEndOfFile()
        let errData = err.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            let message = String(decoding: errData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            throw PackError("词库包解不开：\(message.isEmpty ? "tar 出错" : message)")
        }
        return String(decoding: data, as: UTF8.self)
    }
}

/// 带进度的下载（URLSession 的下载任务），下到临时文件。
private final class DownloadTask: NSObject, URLSessionDownloadDelegate {
    private let url: URL
    private let progress: (Int64, Int64) -> Void
    private let done = DispatchSemaphore(value: 0)
    private var result: Result<URL, Error> = .failure(PackError("下载没完成"))

    init(url: URL, progress: @escaping (Int64, Int64) -> Void) {
        self.url = url
        self.progress = progress
    }

    func run() throws -> URL {
        let session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)
        var request = URLRequest(url: url, timeoutInterval: 60)
        request.setValue("ArchType", forHTTPHeaderField: "User-Agent")
        session.downloadTask(with: request).resume()
        done.wait()
        session.finishTasksAndInvalidate()
        return try result.get()
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        progress(totalBytesWritten, max(totalBytesExpectedToWrite, 0))
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        if let http = downloadTask.response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            result = .failure(PackError("下载失败：GitHub 返回 \(http.statusCode)"))
            return
        }
        // location 在这个回调返回后就被删掉，先挪走
        let keep = FileManager.default.temporaryDirectory
            .appendingPathComponent("archtype-\(UUID().uuidString)-\(url.lastPathComponent)")
        do {
            try FileManager.default.moveItem(at: location, to: keep)
            result = .success(keep)
        } catch {
            result = .failure(error)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let error {
            result = .failure(error)
        }
        done.signal()
    }
}
