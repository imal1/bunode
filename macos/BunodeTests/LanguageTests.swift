import Foundation
import Testing
@testable import Bunode

/// Settings › General › Language (ADR-0006).
struct LanguageTests {
    @Test func theChoiceReflectsAppleLanguages() {
        #expect(AppLanguage(appleLanguages: nil) == .system)
        #expect(AppLanguage(appleLanguages: []) == .system)
        #expect(AppLanguage(appleLanguages: ["en"]) == .english)
        #expect(AppLanguage(appleLanguages: ["en-GB", "zh-Hans"]) == .english)
        #expect(AppLanguage(appleLanguages: ["zh-Hans"]) == .simplifiedChinese)
        #expect(AppLanguage(appleLanguages: ["zh-Hans-CN"]) == .simplifiedChinese)
        #expect(AppLanguage(appleLanguages: ["zh-CN"]) == .simplifiedChinese)
        // No localization for these: Bunode follows the system for them.
        #expect(AppLanguage(appleLanguages: ["zh-Hant"]) == .system)
        #expect(AppLanguage(appleLanguages: ["zh-HK"]) == .system)
        #expect(AppLanguage(appleLanguages: ["fr"]) == .system)
    }

    @Test func eachChoiceResolvesToALocalization() {
        let available = ["en", "zh-Hans"]
        let chinese = ["zh-Hans-CN", "en-US"]
        #expect(AppLanguage.system.resolved(systemLanguages: chinese, available: available) == "zh-Hans")
        #expect(AppLanguage.system.resolved(systemLanguages: ["fr-FR"], available: available) == "en")
        #expect(AppLanguage.english.resolved(systemLanguages: chinese, available: available) == "en")
        #expect(AppLanguage.simplifiedChinese.resolved(systemLanguages: ["en-US"], available: available) == "zh-Hans")
    }

    @Test func choosingWritesTheAppsOwnDefaultAndSystemRemovesIt() throws {
        let domain = "io.github.imal1.BunodeTests.language.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: domain))
        defer { defaults.removePersistentDomain(forName: domain) }
        let preference = LanguagePreference(defaults: defaults, domain: domain)
        #expect(preference.choice == .system)

        preference.choice = .simplifiedChinese
        #expect(defaults.persistentDomain(forName: domain)?["AppleLanguages"] as? [String] == ["zh-Hans"])
        #expect(preference.choice == .simplifiedChinese)

        preference.choice = .system
        #expect(defaults.persistentDomain(forName: domain)?["AppleLanguages"] == nil)
        #expect(preference.choice == .system)
    }

    @Test func restartIsNeededOnlyWhenTheLanguageWouldChange() throws {
        let domain = "io.github.imal1.BunodeTests.language.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: domain))
        defer { defaults.removePersistentDomain(forName: domain) }
        let preference = LanguagePreference(
            defaults: defaults,
            domain: domain,
            systemLanguages: ["en-US"],
            available: ["en", "zh-Hans"],
            runningLocalization: "en"
        )
        #expect(!preference.needsRestart(for: .system))
        #expect(!preference.needsRestart(for: .english))
        #expect(preference.needsRestart(for: .simplifiedChinese))
    }
}
