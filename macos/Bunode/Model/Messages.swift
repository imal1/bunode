import Foundation

// The core reports facts, not sentences (requirements §4.1). These are the
// words the app shows for them, from the String Catalog.

extension CoreError {
    var message: String {
        switch self {
        case .NotAFolder(let path):
            String(localized: "\(path) is not a folder.")
        case .Missing(let path):
            String(localized: "\(path) doesn't exist.")
        case .AlreadyAProject(let path):
            String(localized: "\(path) is already a Project.")
        case .OutsideProject:
            String(localized: "Choose the Project's folder or a folder inside it.")
        case .Save(let path, let reason):
            String(localized: "Couldn't save \(path): \(reason)")
        case .UnknownProject:
            String(localized: "This Project is no longer in Bunode.")
        case .UnknownService:
            String(localized: "This Service is no longer in Bunode.")
        case .EmptyCommand:
            String(localized: "Enter a command.")
        case .UnmatchedQuote:
            String(localized: "The command has an unmatched quote.")
        case .InvalidPort:
            String(localized: "Use a port from 1 to 65535.")
        case .EmptyName:
            String(localized: "Enter a name.")
        case .NameTooLong(let max):
            String(localized: "Use at most \(Int(max)) characters.")
        case .NameTaken(let project):
            String(localized: "Name already used in \(project)")
        case .NotExecutable(let path):
            String(localized: "\(path) is not an executable file.")
        case .InvalidForceStopAfter(let choices):
            String(localized: "Force stop after must be one of \(Self.list(choices)) seconds.")
        case .ProcessExited(let pid):
            String(localized: "PID \(String(pid)) has already exited.")
        case .SignalFailed(let pid, let reason):
            String(localized: "Couldn't send SIGTERM to PID \(String(pid)): \(reason)")
        case .AgentServerSurvived(let name):
            String(localized: "Couldn’t stop \(name).")
        }
    }

    /// `[2, 5, 10]`
    private static func list(_ choices: [UInt32]) -> String {
        "[" + choices.map { String($0) }.joined(separator: ", ") + "]"
    }
}

extension Error {
    /// The message to show the user.
    var userMessage: String {
        if let error = self as? CoreError {
            return error.message
        }
        return localizedDescription
    }
}

extension RunNote {
    /// How a Run ended or why it couldn't start: “Exited with code 1”.
    var message: String {
        switch self {
        case .exited:
            String(localized: "Exited")
        case .exitedWithCode(let code):
            String(localized: "Exited with code \(String(code))")
        case .endedBySignal(let signal):
            String(localized: "Ended by \(signal)")
        case .unmatchedQuote:
            String(localized: "The command has an unmatched quote.")
        case .emptyCommand:
            String(localized: "The command is empty.")
        case .folderMissing(let path):
            String(localized: "\(path) is missing.")
        case .notInPath(let program):
            String(localized: "\(program) wasn't found in PATH.")
        case .programMissing(let program):
            String(localized: "\(program) doesn't exist.")
        case .couldNotStart(let program, let reason):
            String(localized: "Couldn't start \(program): \(reason)")
        case .runnerFailed(let reason):
            String(localized: "Couldn't start bunode-runner: \(reason)")
        case .runnerExited(let status):
            String(localized: "bunode-runner exited unexpectedly (\(status)).")
        case .runnerLost(let reason):
            String(localized: "Lost track of bunode-runner: \(reason)")
        case .unknownEnd:
            String(localized: "The Run ended for an unknown reason.")
        }
    }
}

extension EnvironmentProblem {
    /// Shown in Settings › Advanced › Shell environment.
    /// Whole sentences, so translations needn't nest fragments.
    var message: String {
        switch self {
        case .shellFailed(let shell, let reason):
            String(localized: "Couldn't read the environment (couldn't start \(shell): \(reason)). Runs get Bunode's own.")
        case .noOutput(let shell):
            String(localized: "Couldn't read the environment (\(shell) printed no environment). Runs get Bunode's own.")
        case .timedOut(let shell, let seconds):
            String(localized: "Couldn't read the environment (\(shell) took longer than \(String(seconds)) s). Runs get Bunode's own.")
        case .readFailed(let reason):
            String(localized: "Couldn't read the environment (\(reason)). Runs get Bunode's own.")
        }
    }
}

extension ConfigIssue {
    /// Shown above the Projects list in Settings.
    var message: String {
        switch (problem, backup) {
        case (.unreadable(let detail), let backup?):
            String(localized: "\(path) couldn't be read (\(detail)). It was moved to \(backup) and Bunode started with no Projects.")
        case (.unreadable(let detail), nil):
            String(localized: "\(path) couldn't be read (\(detail)). Bunode started with no Projects.")
        case (.newerFormat(let version), let backup?):
            String(localized: "\(path) was written by a newer Bunode (format \(String(version))). It was moved to \(backup) and Bunode started with no Projects.")
        case (.newerFormat(let version), nil):
            String(localized: "\(path) was written by a newer Bunode (format \(String(version))). Bunode started with no Projects.")
        }
    }
}

/// The app a process was started from, such as “Terminal”.
func sourceName(_ source: String?) -> String {
    source ?? String(localized: "Unknown")
}
