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
        ] {
            #expect(chinese.localizedString(forKey: english, value: nil, table: nil) == simplifiedChinese)
        }
    }
}
