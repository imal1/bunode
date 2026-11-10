import Foundation

/// The interface language picked in Settings › General (ADR-0006). It
/// applies the next time Bunode launches.
enum AppLanguage: Hashable, Sendable {
    case system, english, simplifiedChinese

    /// The choice an `AppleLanguages` override stands for; none is System.
    /// Bunode has no localization for another language, so it shows as
    /// System.
    init(appleLanguages: [String]?) {
        // Maximal, so "zh-CN" gets its script and "zh-HK" its traditional one.
        let language = appleLanguages?.first.map {
            Locale.Language(identifier: Locale.Language(identifier: $0).maximalIdentifier)
        }
        switch (language?.languageCode, language?.script) {
        case (.english, _): self = .english
        case (.chinese, .hanSimplified): self = .simplifiedChinese
        default: self = .system
        }
    }

    /// What the app's `AppleLanguages` is set to; `nil` removes it.
    var appleLanguages: [String]? {
        switch self {
        case .system: nil
        case .english: ["en"]
        case .simplifiedChinese: ["zh-Hans"]
        }
    }

    /// The localization Bunode shows with this choice.
    func resolved(systemLanguages: [String], available: [String]) -> String {
        Bundle.preferredLocalizations(
            from: available,
            forPreferences: appleLanguages ?? systemLanguages
        ).first ?? "en"
    }
}

private let appleLanguagesKey = "AppleLanguages"

/// The app's own `AppleLanguages` default, which macOS reads at launch.
struct LanguagePreference {
    private let defaults: UserDefaults
    private let domain: String
    private let systemLanguages: [String]
    private let available: [String]
    private let runningLocalization: String

    init(
        defaults: UserDefaults = .standard,
        domain: String = Bundle.main.bundleIdentifier ?? "io.github.imal1.Bunode",
        // The global domain holds the system's list; the app's own
        // override is not in it.
        systemLanguages: [String] = UserDefaults.standard
            .persistentDomain(forName: UserDefaults.globalDomain)?[appleLanguagesKey] as? [String]
            ?? Locale.preferredLanguages,
        available: [String] = Bundle.main.localizations.filter { $0 != "Base" },
        runningLocalization: String = Bundle.main.preferredLocalizations.first ?? "en"
    ) {
        self.defaults = defaults
        self.domain = domain
        self.systemLanguages = systemLanguages
        self.available = available
        self.runningLocalization = runningLocalization
    }

    var choice: AppLanguage {
        get {
            // Only the app's own domain: `defaults.array(forKey:)` would fall
            // back to the system's list.
            AppLanguage(appleLanguages: defaults.persistentDomain(forName: domain)?[appleLanguagesKey] as? [String])
        }
        nonmutating set {
            if let languages = newValue.appleLanguages {
                defaults.set(languages, forKey: appleLanguagesKey)
            } else {
                defaults.removeObject(forKey: appleLanguagesKey)
            }
        }
    }

    /// Whether `choice` shows another language than the running app.
    func needsRestart(for choice: AppLanguage) -> Bool {
        choice.resolved(systemLanguages: systemLanguages, available: available) != runningLocalization
    }
}
