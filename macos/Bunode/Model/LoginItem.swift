import Observation
import ServiceManagement

/// Whether macOS opens Bunode when the user logs in, as macOS has it.
enum LoginItemStatus: Equatable {
    case off
    case on
    /// Registered, but the user has to allow it in System Settings.
    case needsApproval
}

/// macOS's login item for Bunode. Tests use a stand-in, so they never
/// touch the user's real login items.
@MainActor
protocol LoginItemService {
    var status: LoginItemStatus { get }
    func register() throws
    func unregister() throws
    /// System Settings › General › Login Items & Extensions.
    func openSystemSettings()
}

/// The app itself as a login item (`SMAppService.mainApp`): opened after
/// the user logs in, never before.
struct SystemLoginItem: LoginItemService {
    var status: LoginItemStatus {
        switch SMAppService.mainApp.status {
        case .enabled: .on
        case .requiresApproval: .needsApproval
        // An app that was never registered reads as "not found".
        case .notRegistered, .notFound: .off
        @unknown default: .off
        }
    }

    func register() throws {
        try SMAppService.mainApp.register()
    }

    func unregister() throws {
        try SMAppService.mainApp.unregister()
    }

    func openSystemSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}

/// Settings › General › Open at login. The switch shows what macOS says,
/// never what was asked for: a registration that failed stays off, with
/// the reason. It only opens Bunode; which Services start then is each
/// Service's own Start with Bunode.
@MainActor
@Observable
final class LoginItemModel {
    private(set) var status: LoginItemStatus
    /// Why the last change didn't take.
    private(set) var problem: String?

    @ObservationIgnored private let service: any LoginItemService

    init(service: (any LoginItemService)? = nil) {
        let service = service ?? SystemLoginItem()
        self.service = service
        status = service.status
    }

    var isOn: Bool { status == .on }

    /// Reads the status again: the user may have changed it in System
    /// Settings.
    func refresh() {
        let current = service.status
        if current != status {
            status = current
            problem = nil
        }
    }

    func set(_ on: Bool) {
        do {
            if on {
                try service.register()
            } else {
                try service.unregister()
            }
            problem = nil
        } catch {
            problem = on
                ? String(localized: "Couldn’t add Bunode to Login Items: \(error.localizedDescription)")
                : String(localized: "Couldn’t remove Bunode from Login Items: \(error.localizedDescription)")
        }
        status = service.status
    }

    func openSystemSettings() {
        service.openSystemSettings()
    }
}
