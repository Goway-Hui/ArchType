// 整句增强模型（字级神经语言模型）：从微信输入法导入 nlrgpt2_nnlm.bin（和 Linux 设置的「整句增强模型」一样）。
//
// 模型是腾讯的，ArchType 不分发它。用户选微信输入法 macOS 版的安装包（.zip）、装好的 WeType.app 或模型文件本身，
// 从里面找 nlrgpt2_nnlm.bin（zip 用系统的 ditto 解到临时目录），写成临时文件，核心试着解析过（archtype_nnlm_check）
// 才换到 ~/Library/Application Support/ArchType/wetype/nlrgpt2_nnlm.bin。不看版本号、MD5：解析得了就能用。
// 输入法看到这个文件变了（AppState.checkForChanges）就在后台重新载入引擎，不用重新登录。
// 没导入时用安装包里的 nnlm.bin（ArchType 自己训的整句增强模型，效果和微信输入法的相近）。

import CArchtype
import Darwin
import Foundation

enum NnlmInstaller {
    static let fileName = "nlrgpt2_nnlm.bin"

    /// 导入的模型放这里（同 Linux 的 ~/.local/share/archtype/wetype/nlrgpt2_nnlm.bin）
    static func model(in supportDir: URL) -> URL {
        supportDir.appendingPathComponent("wetype", isDirectory: true).appendingPathComponent(fileName)
    }

    /// 核心试着解析：返回字表大小，不是能用的模型是 nil
    static func check(_ path: String) -> Int? {
        let size = Int(archtype_nnlm_check(path))
        return size >= 0 ? size : nil
    }

    /// 从 source（.zip、.app 或别的目录、模型文件本身）导入，返回字表大小。出错时原来导入的不动。
    @discardableResult
    static func install(from source: URL, supportDir: URL) throws -> Int {
        let fm = FileManager.default
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: source.path, isDirectory: &isDir) else {
            throw PackError("找不到 \(source.path)")
        }
        var unpacked: URL?
        defer {
            if let unpacked { try? fm.removeItem(at: unpacked) }
        }
        let found: URL
        if isDir.boolValue {
            guard let hit = find(in: source) else {
                throw PackError("\(source.lastPathComponent) 里没有 \(fileName)")
            }
            found = hit
        } else if isZip(source) {
            let dir = fm.temporaryDirectory.appendingPathComponent("archtype-nnlm-\(UUID().uuidString)", isDirectory: true)
            unpacked = dir
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
            do {
                try LocalModelInstaller.run("/usr/bin/ditto", ["-x", "-k", source.path, dir.path], timeout: 600)
            } catch let error as PackError {
                throw PackError("解不开 \(source.lastPathComponent)：\(error.description)")
            }
            guard let hit = find(in: dir) else {
                throw PackError("\(source.lastPathComponent) 里没有 \(fileName)")
            }
            found = hit
        } else {
            found = source
        }

        let target = model(in: supportDir)
        try fm.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
        // 先写到旁边的临时文件（名字不一样，输入法不会读到写了一半的），解析得了才换上去
        let staging = target.deletingLastPathComponent().appendingPathComponent(fileName + ".new")
        try? fm.removeItem(at: staging)
        // 读出来再写：选的是链接时也拷真的内容
        try Data(contentsOf: found).write(to: staging)
        guard let vocab = check(staging.path) else {
            try? fm.removeItem(at: staging)
            throw PackError("\(found.lastPathComponent) 不是能用的整句增强模型（解析不了），没有导入")
        }
        guard rename(staging.path, target.path) == 0 else {
            let reason = String(cString: strerror(errno))
            try? fm.removeItem(at: staging)
            throw PackError("模型换不上去：\(reason)")
        }
        return vocab
    }

    /// 删掉导入的模型，返回空出的字节数（安装包里的不动）。
    @discardableResult
    static func remove(supportDir: URL) -> Int64 {
        let path = model(in: supportDir).path
        let size = LocalModelInstaller.fileSize(path)
        try? FileManager.default.removeItem(atPath: path)
        return size
    }

    /// 目录（包括 .app 里面）里第一个叫 nlrgpt2_nnlm.bin 的普通文件
    private static func find(in dir: URL) -> URL? {
        guard let items = FileManager.default.enumerator(at: dir, includingPropertiesForKeys: [.isRegularFileKey])
        else { return nil }
        for case let item as URL in items where item.lastPathComponent == fileName {
            if (try? item.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true {
                return item
            }
        }
        return nil
    }

    /// zip 文件：看开头的 PK\3\4，看不了就看扩展名
    private static func isZip(_ url: URL) -> Bool {
        if let handle = try? FileHandle(forReadingFrom: url) {
            defer { try? handle.close() }
            if let head = try? handle.read(upToCount: 4), head.count == 4 {
                return head == Data([0x50, 0x4B, 0x03, 0x04])
            }
        }
        return url.pathExtension.lowercased() == "zip"
    }
}
