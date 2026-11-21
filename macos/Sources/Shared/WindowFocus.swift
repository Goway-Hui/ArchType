// 换到别的应用、别的窗口了没有（WindowMode，#254）。外壳在输入控制器启用时把「哪个应用的哪个窗口」交过来，
// 和上一次启用时的比：不一样就按 WindowMode 定一次中 / 英。同一个窗口里换输入框不算（Linux 上一个窗口一个输入上下文）。
// 不依赖 AppKit，测试编得到。

public struct WindowFocus {
    /// 上一次启用 ArchType 的应用和窗口；还没有过是 nil
    public private(set) var last: String?

    public init() {}

    /// 应用（bundle id）和它最前面的窗口号拼成一个键；问不到窗口号时只看应用。
    public static func key(app: String, window: Int?) -> String {
        guard let window else { return app }
        return "\(app)#\(window)"
    }

    /// 记下这一次的应用和窗口，返回是不是换了（第一次也算换了）。
    public mutating func moved(to key: String) -> Bool {
        defer { last = key }
        return key != last
    }
}
