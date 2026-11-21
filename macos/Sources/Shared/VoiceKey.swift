// 语音键：设置里的 VoiceKey 用 fcitx5 的键名（和 Linux 一样，一份 archtype.conf 两边通用），
// 这里翻成 macOS 的键码。修饰键（右 Option 等）在 flagsChanged 里看按下 / 松开，F 键看 keyDown / keyUp。
// 不依赖 Foundation / AppKit：按键测试在 Linux 上也编。

public struct VoiceKey: Equatable, Sendable {
    /// macOS 虚拟键码（kVK_*）
    public let keyCode: UInt16
    /// 修饰键在 NSEvent.modifierFlags 里的「分左右」那一位（NX_DEVICE*KEYMASK）；不是修饰键为 0
    public let deviceMask: UInt
    /// 同一种修饰键左右两边的设备位（Option 是 0x20 | 0x40）；不是修饰键为 0
    public let sidesMask: UInt
    /// 不分左右的修饰键位（NSEvent.ModifierFlags.option 等）；不是修饰键为 0
    public let genericMask: UInt
    /// 设置里写的键名
    public let name: String

    public var isModifier: Bool { deviceMask != 0 }

    /// fcitx5 键名 → 键；留空、none、Mac 上没有的键（Menu）返回 nil，语音输入就不用热键。
    public init?(name: String) {
        guard let (code, mask, sides, generic) = VoiceKey.table[name] else { return nil }
        keyCode = code
        deviceMask = mask
        sidesMask = sides
        genericMask = generic
        self.name = name
    }

    /// 这次 flagsChanged 里语音键是不是按着（rawFlags 是 NSEvent.modifierFlags.rawValue）。
    /// 有的键盘、远程桌面、键位映射工具发来的事件不带分左右的设备位，只有 Option 这种通用位（#231）：
    /// 这时看通用位（调用的地方已经按键码认准了是右边那个键），和 Shift 的做法一样。
    public func isDown(rawFlags: UInt) -> Bool {
        rawFlags & sidesMask != 0 ? rawFlags & deviceMask != 0 : rawFlags & genericMask != 0
    }

    // 键码见 Carbon 的 Events.h（kVK_RightOption 等），分左右的位见 IOKit 的 NX_DEVICE*KEYMASK，
    // 通用位是 NSEvent.ModifierFlags 的 control 0x40000、option 0x80000、command 0x100000
    private static let table: [String: (UInt16, UInt, UInt, UInt)] = [
        "Alt_R": (0x3D, 0x40, 0x60, 0x80000), "Alt_L": (0x3A, 0x20, 0x60, 0x80000),
        "Control_R": (0x3E, 0x2000, 0x2001, 0x40000), "Control_L": (0x3B, 0x01, 0x2001, 0x40000),
        "Super_R": (0x36, 0x10, 0x18, 0x100000), "Super_L": (0x37, 0x08, 0x18, 0x100000),
        "F1": (0x7A, 0, 0, 0), "F2": (0x78, 0, 0, 0), "F3": (0x63, 0, 0, 0), "F4": (0x76, 0, 0, 0),
        "F5": (0x60, 0, 0, 0), "F6": (0x61, 0, 0, 0), "F7": (0x62, 0, 0, 0), "F8": (0x64, 0, 0, 0),
        "F9": (0x65, 0, 0, 0), "F10": (0x6D, 0, 0, 0), "F11": (0x67, 0, 0, 0), "F12": (0x6F, 0, 0, 0),
    ]

    /// 设置窗口里能选的（和 spec/config.json 里 VoiceKey 的 presets.macos 一样）
    public static let presets: [(label: String, name: String)] = [
        ("右 Option", "Alt_R"), ("右 Control", "Control_R"), ("F8", "F8"), ("F9", "F9"), ("F10", "F10"),
    ]
}

/// 正在听时音量画成一排竖条（和 Linux 插件的 voiceWave 一样，不足 6 个时左边补最低的）。
public func voiceWave(_ levels: [Int]) -> String {
    let bars = Array("▁▂▃▄▅▆▇█")
    let shown = levels.suffix(6)
    var out = String(repeating: bars[0], count: 6 - shown.count)
    for level in shown {
        out.append(bars[min(max(level, 0), 100) * (bars.count - 1) / 100])
    }
    return out
}
