// 诊断信息（设置窗口「帮助」→「复制诊断信息」）里的设置文件：贴给别人看之前把可能是密钥的值打码，
// 和 Linux 设置的 collect_diagnostics、mask_url 一样。收集文件、版本这些在 App 层（Diagnostics）。
// 不依赖 Foundation，测试在 Linux 上也能跑。

public enum DiagnosticsText {
    /// 设置文件原文打码：TranslateBaseUrl 只留协议和主机（有的代理把 Key 放在路径里）；
    /// 不认识的键名字里带 key、token、secret、password 的（API Key 写错了地方）整个打码。注释、[分组] 原样。
    public static func maskConfig(_ text: String) -> String {
        text.split(separator: "\n", omittingEmptySubsequences: false).map { raw -> String in
            let line = String(raw)
            let body = line.drop(while: { $0 == " " || $0 == "\t" })
            guard !body.hasPrefix("#"), !body.hasPrefix(";"), !body.hasPrefix("["),
                  let eq = line.firstIndex(of: "=")
            else { return line }
            let key = String(line[..<eq]).filter { $0 != " " && $0 != "\t" }
            let value = trimmed(String(line[line.index(after: eq)...]))
            if key == "TranslateBaseUrl" {
                return key + "=" + maskURL(value)
            }
            if !ArchtypeConfig.keys.contains(key), looksSecret(key), !value.isEmpty, value != "\"\"" {
                return key + "=（已打码）"
            }
            return line
        }.joined(separator: "\n")
    }

    /// 网址只留协议和主机：https://user:key@api.example.com/v1/xxx → https://api.example.com/…；空的还是空的
    public static func maskURL(_ url: String) -> String {
        var chars = Array(trimmed(url))
        if chars.count >= 2, chars.first == "\"", chars.last == "\"" {
            chars = Array(chars.dropFirst().dropLast())
        }
        guard !chars.isEmpty else { return "" }
        var schemeEnd = -1
        var i = 0
        while i + 2 < chars.count {
            if chars[i] == ":" && chars[i + 1] == "/" && chars[i + 2] == "/" {
                schemeEnd = i
                break
            }
            i += 1
        }
        guard schemeEnd > 0 else { return "（已打码）" }
        let netloc = chars[(schemeEnd + 3)...].prefix { $0 != "/" && $0 != "?" && $0 != "#" }
        let host = netloc.split(separator: "@", omittingEmptySubsequences: false).last.map { String($0) } ?? ""
        guard !host.isEmpty else { return "（已打码）" }
        return String(chars[..<schemeEnd]) + "://" + host + "/…"
    }

    private static func looksSecret(_ key: String) -> Bool {
        let lower = Array(key.lowercased())
        return ["key", "token", "secret", "password"].contains { word in
            let needle = Array(word)
            guard lower.count >= needle.count else { return false }
            return (0...(lower.count - needle.count)).contains { Array(lower[$0..<($0 + needle.count)]) == needle }
        }
    }

    private static func trimmed(_ s: String) -> String {
        var s = Substring(s)
        while let c = s.first, c == " " || c == "\t" || c == "\r" { s.removeFirst() }
        while let c = s.last, c == " " || c == "\t" || c == "\r" { s.removeLast() }
        return String(s)
    }
}
