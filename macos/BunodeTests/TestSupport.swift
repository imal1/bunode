import Foundation

/// A fresh temporary directory, removed when the value is deinitialized.
final class TemporaryDirectory {
    let url: URL

    init() {
        url = FileManager.default.temporaryDirectory
            .appending(path: "BunodeTests-\(UUID().uuidString)", directoryHint: .isDirectory)
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    func folder(_ name: String, files: [String: String] = [:]) -> URL {
        let folder = url.appending(path: name, directoryHint: .isDirectory)
        try! FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        for (file, contents) in files {
            try! contents.write(to: folder.appending(path: file), atomically: true, encoding: .utf8)
        }
        return folder
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }
}
