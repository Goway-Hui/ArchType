// 候选框摆在屏幕的哪里（CandidatePanel 用）：每次都照光标这一行重新算，夹在屏幕的可见区域里。
//
// 不依赖 AppKit / Foundation：按键测试在 Linux 上也编。坐标和 AppKit 一样是屏幕坐标、左下角为原点、单位点。

/// 一个矩形（AppKit 的 NSRect 换成 Double）。
public struct PanelRect: Equatable {
    public var x: Double
    public var y: Double
    public var width: Double
    public var height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }

    public var minX: Double { x }
    public var maxX: Double { x + width }
    public var minY: Double { y }
    public var maxY: Double { y + height }
}

public enum PanelPlacement {
    /// 候选框（width × height）的左下角。line 是光标所在行（应用给的，每次都从它算起，不看上一次摆在哪），
    /// visible 是这块屏幕去掉菜单栏、程序坞的可见区域；leading 是候选框左边到第一个候选的序号有多宽
    /// （序号对着编辑区的第一个字母），gap 是这一行和候选框之间的空隙。
    ///
    /// 下面放不下放到这一行上面。右边放不下往左挪，但左边不出可见区域：候选框比屏幕还宽时宁可右边出去，
    /// 第一个候选总看得见（#227：以前先夹左边、再夹右边，长句的候选把框撑得比屏幕宽时整个往左推，
    /// 越打越往左，第一个候选被挤出屏幕）。上下也一样，太高时顶上（第一行）留在屏幕里。
    public static func origin(width: Double, height: Double, line: PanelRect, visible: PanelRect,
                              leading: Double, gap: Double) -> (x: Double, y: Double) {
        var x = line.minX - leading
        var y = line.minY - height - gap
        if y < visible.minY {
            y = line.maxY + gap
        }
        x = max(min(x, visible.maxX - width), visible.minX)
        y = min(max(y, visible.minY), visible.maxY - height)
        return (x, y)
    }
}
