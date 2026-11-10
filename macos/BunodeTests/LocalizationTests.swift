import Foundation
import Testing
@testable import Bunode

/// Counts go through the String Catalog's plural rules, never through
/// string concatenation (design-handoff §7).
struct LocalizationTests {
    @Test func servicesArePluralized() {
        #expect(String(localized: "\(1) Services") == "1 Service")
        #expect(String(localized: "\(2) Services") == "2 Services")
        #expect(String(localized: "\(0) Services") == "0 Services")
        #expect(String(localized: "\(1) variables from \("/bin/zsh") · \("just now")") == "1 variable from /bin/zsh · just now")
    }

    @Test func restartTextIsPluralized() {
        #expect(String(localized: "Stop \(1) Services and Restart") == "Stop 1 Service and Restart")
        #expect(String(localized: "\(2) Services are running. Bunode stops every Run before it restarts.") == "2 Services are running. Bunode stops every Run before it restarts.")
    }

    @Test func simplifiedChineseEntriesAreShipped() throws {
        let path = try #require(Bundle.main.path(forResource: "zh-Hans", ofType: "lproj"))
        let chinese = try #require(Bundle(path: path))
        #expect(chinese.localizedString(forKey: "Restart Now", value: nil, table: nil) == "立即重启")
        #expect(chinese.localizedString(forKey: "General", value: nil, table: nil) == "通用")
        for (english, simplifiedChinese) in [
            ("List", "列表"), ("How the menu bar list is arranged.", "菜单栏列表的排列方式。"),
            ("Flat", "平铺"), ("By Project", "按项目"),
            ("Show dev servers started by agents", "显示 agent 启动的开发服务"),
            ("Started by %@", "由 %@ 启动"),
            ("Open in Browser", "在浏览器中打开"), ("Show in Finder", "在访达中显示"),
            ("Couldn’t stop %@.", "无法停止 %@。"), ("Stopping…", "正在停止…"),
            // design-handoff §6.6
            ("Add Service", "添加服务"), ("Settings", "设置"), ("Quit", "退出"),
            ("Start", "启动"), ("Stop", "停止"), ("Restart", "重启"), ("Run", "运行"),
            ("Starting…", "正在启动…"), ("Restarting…", "正在重启…"), ("Stopped", "已停止"),
            ("Crashed", "已崩溃"), ("Failed", "失败"), ("Not responding", "无响应"),
            ("Folder missing", "目录不存在"), ("Running…", "运行中…"), ("Done", "已完成"),
            ("Other processes", "其他进程"), ("%lld started outside Bunode", "%lld 个在 Bunode 外启动"),
            ("None right now", "暂无"),
            // Typed errors and how a Run ended
            ("Exited with code %@", "已退出，代码 %@"), ("%@ wasn't found in PATH.", "在 PATH 中找不到 %@。"),
            ("The command has an unmatched quote.", "命令中的引号不成对。"),
        ] {
            #expect(chinese.localizedString(forKey: english, value: nil, table: nil) == simplifiedChinese)
        }
    }

    @Test func chinesePluralsKeepTheCount() throws {
        let path = try #require(Bundle.main.path(forResource: "zh-Hans", ofType: "lproj"))
        let chinese = try #require(Bundle(path: path))
        let format = chinese.localizedString(forKey: "%lld lines", value: nil, table: nil)
        #expect(String.localizedStringWithFormat(format, 1) == "1 行")
        #expect(String.localizedStringWithFormat(format, 3) == "3 行")
    }

    /// Every String Catalog entry ships Simplified Chinese (#34).
    @Test func everyCatalogEntryHasSimplifiedChinese() throws {
        let catalog = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appending(path: "Bunode/Localizable.xcstrings")
        let json = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: catalog)) as? [String: Any])
        let strings = try #require(json["strings"] as? [String: Any])
        let untranslated = strings.keys.filter { key in
            let entry = strings[key] as? [String: Any]
            let zh = (entry?["localizations"] as? [String: Any])?["zh-Hans"]
            return !Self.isTranslated(zh)
        }.sorted()
        #expect(untranslated.isEmpty, "No zh-Hans: \(untranslated)")

        // A Chinese value fills in the same arguments as its English key.
        let mismatched = strings.keys.filter { key in
            let entry = strings[key] as? [String: Any]
            let zh = (entry?["localizations"] as? [String: Any])?["zh-Hans"] as? [String: Any]
            let unit = zh?["stringUnit"] as? [String: Any]
            guard let value = unit?["value"] as? String else { return false }
            return Self.specifiers(value) != Self.specifiers(key)
        }.sorted()
        #expect(mismatched.isEmpty, "Format specifiers differ: \(mismatched)")
    }

    private static func specifiers(_ format: String) -> [String] {
        format.matches(of: /%(?:\d+\$)?(?:lld|ld|d|u|@)/).map { String($0.output) }.sorted()
    }

    /// A stringUnit, or plural variations whose every unit, is translated.
    private static func isTranslated(_ localization: Any?) -> Bool {
        guard let localization = localization as? [String: Any] else { return false }
        if let unit = localization["stringUnit"] as? [String: Any] {
            return unit["state"] as? String == "translated" && !((unit["value"] as? String) ?? "").isEmpty
        }
        guard let variations = localization["variations"] as? [String: Any], !variations.isEmpty else { return false }
        return variations.values.allSatisfy { cases in
            guard let cases = cases as? [String: Any], !cases.isEmpty else { return false }
            return cases.values.allSatisfy(isTranslated)
        }
    }
}
