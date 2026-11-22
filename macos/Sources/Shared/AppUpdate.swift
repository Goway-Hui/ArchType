// 检查更新的规矩（macOS）：发布页的 tag 怎么变成版本号、macOS 安装包叫什么、什么时候算有新版、多久查一次。
// 包名和 install.sh 的 macOS 分支一样（ArchType-<版本>-macos.pkg，arm64 + x86_64 通用），版本号的写法也照它认；
// 比版本用 LexiconPack.compareVersions（按数字段逐段比，和 Linux 设置的 parse_version 一样）。
// 下载、校验、交给系统的 installer 在 App 层（UpdateInstaller），这里不依赖 Foundation，测试在 Linux 上也能跑。

public enum AppUpdate {
    /// 发布页（出错时给用户的链接）
    public static let releasesPage = "https://github.com/Goway-Hui/ArchType/releases"
    /// 输入法进程最多隔这么久查一次（秒）：一天
    public static let checkInterval: Int64 = 24 * 3600

    /// tag（v0.4.1 或 0.4.1）里的版本号；不像版本号的（空的、带 / 空格的）是 nil。和 install.sh 一样：
    /// 字母或数字开头，后面只有字母、数字和 . _ + ~ ^ -
    public static func version(tag: String) -> String? {
        var v = Substring(tag)
        if v.hasPrefix("v") { v = v.dropFirst() }
        guard let first = v.unicodeScalars.first, isAlnum(first),
              v.unicodeScalars.allSatisfy({ isAlnum($0) || "._+~^-".unicodeScalars.contains($0) })
        else { return nil }
        return String(v)
    }

    /// 发布页上的 macOS 安装包
    public static func packageName(version: String) -> String {
        "ArchType-\(version)-macos.pkg"
    }

    /// 正式版的版本号（数字开头，0.4.0 这样）；开发时编出来的是 dev
    public static func isRelease(_ version: String) -> Bool {
        guard let first = version.unicodeScalars.first else { return false }
        return ("0"..."9").contains(first)
    }

    /// GitHub 发布列表里的一项（Beta 计划挑版本用）
    public struct Listed: Equatable {
        public let tag: String
        public let prerelease: Bool
        public let draft: Bool

        public init(tag: String, prerelease: Bool = false, draft: Bool = false) {
            self.tag = tag
            self.prerelease = prerelease
            self.draft = draft
        }
    }

    /// Beta 计划：发布列表里版本号最大的那个的下标，预发布也算，一样大时要正式版；没有合适的是 nil。
    /// 只看 v 加数字开头的 tag：数据的发布（lm-…、nnlm-…）也是预发布，不是新版本。草稿不算。和 Linux 的 pick_release 一样
    public static func pickNewest(_ releases: [Listed]) -> Int? {
        var best: Int?
        for (i, r) in releases.enumerated() where !r.draft && isAppTag(r.tag) {
            guard let b = best else {
                best = i
                continue
            }
            let c = LexiconPack.compareVersions(r.tag, releases[b].tag)
            if c > 0 || (c == 0 && releases[b].prerelease && !r.prerelease) {
                best = i
            }
        }
        return best
    }

    // MARK: 检查更新走网页

    /// GitHub 的接口不登录时每个 IP 每小时只能问 60 次（校园网、公司网、运营商 NAT 后面很多人共用一个 IP），
    /// 用完了回 403。所以检查更新先走网页：releases/latest 跳到最新正式版的 tag 页，releases.atom 列着最近的发布，
    /// 发布页的 SHA256SUMS 列着这一版的全部文件，RELEASE-NOTES.md 是更新说明。这些都不算接口次数。
    /// 和 Linux 的 fetch_latest_web 一样
    public static let repoPage = "https://github.com/Goway-Hui/ArchType"
    public static let latestPage = repoPage + "/releases/latest"
    public static let atomFeed = repoPage + "/releases.atom"
    /// 发布任务把「这一版要注意」那一节单独传成这个文件（0.7.1 起），取更新说明也不用接口
    public static let notesFile = "RELEASE-NOTES.md"
    /// 更早的版本没有那个文件：有新版时才问一次接口要说明
    public static func tagAPI(_ tag: String) -> String {
        "https://api.github.com/repos/Goway-Hui/ArchType/releases/tags/\(tag)"
    }

    public static func downloadURL(tag: String, file: String) -> String {
        "\(repoPage)/releases/download/\(tag)/\(file)"
    }

    public static func tagPage(_ tag: String) -> String {
        "\(repoPage)/releases/tag/\(tag)"
    }

    /// releases/latest 跳到的地址（…/releases/tag/v0.6.0）里的 tag；不是这个样子是 nil
    public static func tag(fromLatestURL url: String) -> String? {
        guard let range = url.range(of: "/releases/tag/") else { return nil }
        let tag = String(url[range.upperBound...].prefix { $0 != "?" && $0 != "#" && $0 != "/" })
        return tag.isEmpty ? nil : tag
    }

    /// 网页上要按顺序试的 tag：最新正式版，Beta 计划开着时加上 releases.atom 里的版本；只要 v 加数字的，
    /// 版本号大的在前，一样大时正式版在前。atom 里也有只推了 tag、还没发出来的版本，调用方遇到 404 往下试
    public static func webCandidates(latest: String?, feed: String?) -> [String] {
        var tags: [String] = []
        if let latest, isAppTag(latest) { tags.append(latest) }
        if let feed {
            var rest = feed[...]
            while let range = rest.range(of: "/releases/tag/") {
                let tag = String(rest[range.upperBound...].prefix { !"\"'<>/?#".contains($0) })
                if isAppTag(tag) && !tags.contains(tag) { tags.append(tag) }
                rest = rest[range.upperBound...]
            }
        }
        return tags.sorted { a, b in
            let c = LexiconPack.compareVersions(a, b)
            return c != 0 ? c > 0 : a == latest
        }
    }

    /// SHA256SUMS 里列的文件名（「64 位十六进制  文件名」，文件名前可以带 *）
    public static func assetNames(sums: String) -> [String] {
        // 不用 Foundation（这个文件的测试在 Linux 上也跑）：空白自己切
        let blank: (Character) -> Bool = { $0 == " " || $0 == "\t" || $0 == "\r" }
        return sums.split(whereSeparator: \.isNewline).compactMap { line in
            let parts = line.split(maxSplits: 1, omittingEmptySubsequences: true, whereSeparator: blank)
            guard parts.count == 2, parts[0].count == 64, parts[0].allSatisfy(\.isHexDigit) else { return nil }
            var name = parts[1].drop(while: blank)
            while let last = name.last, blank(last) { name = name.dropLast() }
            if name.first == "*" { name = name.dropFirst() }
            return name.isEmpty || name.contains("/") ? nil : String(name)
        }
    }

    /// v0.5.0 这样的 tag（v 加数字和点）
    static func isAppTag(_ tag: String) -> Bool {
        guard tag.hasPrefix("v") || tag.hasPrefix("V") else { return false }
        let rest = tag.unicodeScalars.dropFirst()
        guard let first = rest.first, ("0"..."9").contains(first), rest.last != "." else { return false }
        return rest.allSatisfy { ("0"..."9").contains($0) || $0 == "." }
    }

    /// latest 比现在装的 current 新。current 不是正式版（dev）时不算：开发版不提示更新（和 Linux 一样）
    public static func isNewer(_ latest: String, than current: String) -> Bool {
        isRelease(current) && isRelease(latest) && LexiconPack.compareVersions(latest, current) > 0
    }

    /// 数据目录里 update-state 记的：上次查成功的时间（Unix 秒）、那时发布页上最新的 tag。
    /// 输入法和设置窗口都读写它：一边查过了另一边不用再查，输入法菜单也知道有没有新版。
    public struct State: Equatable {
        public var checked: Int64
        public var latest: String

        public init(checked: Int64 = 0, latest: String = "") {
            self.checked = checked
            self.latest = latest
        }

        /// 一行一项「键=值」；不认识的行、写坏的值不管
        public init(text: String) {
            self.init()
            for line in text.split(whereSeparator: { $0 == "\n" || $0 == "\r\n" || $0 == "\r" }) {
                guard let eq = line.firstIndex(of: "=") else { continue }
                let key = line[..<eq]
                let value = String(line[line.index(after: eq)...])
                switch key {
                case "checked": checked = Int64(value) ?? checked
                case "latest": latest = AppUpdate.version(tag: value) == nil ? latest : value
                default: break
                }
            }
        }

        public var text: String {
            "checked=\(checked)\nlatest=\(latest)\n"
        }
    }

    /// 该查了：上次查过一天了（时钟往回拨了也算）
    public static func due(_ state: State, now: Int64) -> Bool {
        now - state.checked >= checkInterval || now < state.checked
    }

    /// 发布说明里只留「## 这一版」那一节（去掉标题本身）；没有这一节就整个原样（和 Linux 设置的 release_highlights 一样）。
    /// 后面的安装步骤、自动生成的提交列表对已经装好的人没用。
    public static func highlights(_ notes: String) -> String {
        let all = notes.trimmingWhitespace()
        let lines = all.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).map(String.init)
        for (i, line) in lines.enumerated() {
            let stripped = line.trimmingWhitespace()
            let title = String(stripped.dropFirst(2)).trimmingWhitespace()
            // 发布任务写的标题是「这一版要注意」
            guard stripped.hasPrefix("##"), title == "这一版" || title == "这一版要注意" else {
                continue
            }
            var body: [String] = []
            for rest in lines[(i + 1)...] {
                if isSectionHeading(rest) { break }
                body.append(rest)
            }
            let text = body.joined(separator: "\n").trimmingWhitespace()
            return text.isEmpty ? all : text
        }
        return all
    }

    /// 「# 」「## 」开头的一行（一节的开始；### 小标题不算）
    private static func isSectionHeading(_ line: String) -> Bool {
        var rest = Substring(line)
        var hashes = 0
        while hashes < 2, rest.first == "#" {
            hashes += 1
            rest.removeFirst()
        }
        guard hashes > 0, let next = rest.first else { return false }
        return next == " " || next == "\t"
    }

    /// 设置窗口「更新内容」显示的字（Linux 是 markdown_to_pango）：只留「这一版」，标题换成粗体、列表换成 •、
    /// 去掉代码块的 ```（粗体、行内代码、链接留给界面按行内 Markdown 画）。太长的截到 6000 个字。
    public static func notesText(_ notes: String) -> String {
        var out: [String] = []
        var inCode = false
        for raw in highlights(notes).split(omittingEmptySubsequences: false, whereSeparator: \.isNewline) {
            var line = String(raw)
            while line.last == " " || line.last == "\t" {
                line.removeLast()
            }
            if line.trimmingWhitespace().hasPrefix("```") {
                inCode.toggle()
                continue
            }
            if inCode {
                out.append("    " + line)
                continue
            }
            let body = line.drop { $0 == " " || $0 == "\t" }
            let indent = line.count - body.count
            let title = body.drop { $0 == "#" }
            if title.count < body.count, title.first == " " {
                if let last = out.last, !last.isEmpty {
                    out.append("")
                }
                out.append("**" + String(title.dropFirst()).trimmingWhitespace() + "**")
                continue
            }
            if let first = body.first, "-*+".contains(first), body.dropFirst().first == " " {
                out.append(String(repeating: "  ", count: indent / 2) + "• " + String(body.dropFirst(2)))
                continue
            }
            out.append(line)
        }
        let text = out.joined(separator: "\n")
        return text.count > 6000 ? String(text.prefix(6000)) + "…" : text
    }

    private static func isAlnum(_ s: Unicode.Scalar) -> Bool {
        ("0"..."9").contains(s) || ("a"..."z").contains(s) || ("A"..."Z").contains(s)
    }
}
