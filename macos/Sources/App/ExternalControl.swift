// 用命令切中 / 英 / 译（vim 退出插入模式时切英文这类用法），和 Linux 的 org.archtype.Input（会话总线）一样。
//
// macOS 没有会话总线，用分布式通知：命令行（ArchType --mode [zh|en|translate|toggle]）发一个请求，
// 输入法切最近用过的输入框（vim 所在的终端），再把切完的模式发回来；不弹「中 / 英」提示。

import AppKit

enum ExternalControl {
    /// 请求：object 是要切到的模式（zh、en、translate、toggle），空串只问现在的模式
    static let request = Notification.Name("io.github.gloridust.ArchType.SetMode")
    /// 回复：object 是切完以后的模式（zh、en、translate）；还没有输入框用过 ArchType 时是空串
    static let reply = Notification.Name("io.github.gloridust.ArchType.Mode")
    static let modes = ["zh", "en", "translate", "toggle"]
}

/// 输入法进程里收请求的（main.swift 起输入法服务前 start）。
final class ExternalControlListener: NSObject {
    static let shared = ExternalControlListener()

    func start() {
        // 输入法不是前台程序：不立刻送的分布式通知会一直攒着
        DistributedNotificationCenter.default().addObserver(self, selector: #selector(received(_:)),
                                                            name: ExternalControl.request, object: nil,
                                                            suspensionBehavior: .deliverImmediately)
    }

    @objc private func received(_ note: Notification) {
        let request = note.object as? String ?? ""
        let mode = AppState.shared.externalMode(request)
        DistributedNotificationCenter.default().postNotificationName(ExternalControl.reply, object: mode,
                                                                     userInfo: nil, deliverImmediately: true)
    }
}

/// 命令行这边：发请求，等输入法回话（最多 2 秒）。
final class ExternalControlClient: NSObject {
    private var answer: String?

    /// 返回输入法回的模式；输入法没在跑（或者没回）是 nil。
    func send(_ request: String) -> String? {
        let center = DistributedNotificationCenter.default()
        center.addObserver(self, selector: #selector(replied(_:)), name: ExternalControl.reply, object: nil,
                           suspensionBehavior: .deliverImmediately)
        defer { center.removeObserver(self) }
        center.postNotificationName(ExternalControl.request, object: request, userInfo: nil, deliverImmediately: true)
        let deadline = Date().addingTimeInterval(2)
        while answer == nil && Date() < deadline {
            _ = RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        return answer
    }

    @objc private func replied(_ note: Notification) {
        answer = note.object as? String ?? ""
    }
}
