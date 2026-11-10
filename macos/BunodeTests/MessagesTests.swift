import Foundation
import Testing
@testable import Bunode

/// The core reports facts; these are the words the app shows for them
/// (requirements §4.1).
struct MessagesTests {
    @Test func coreErrorsAreWorded() {
        let cases: [(CoreError, String)] = [
            (.NotAFolder(path: "/a"), "/a is not a folder."),
            (.Missing(path: "/a"), "/a doesn't exist."),
            (.AlreadyAProject(path: "/a"), "/a is already a Project."),
            (.OutsideProject, "Choose the Project's folder or a folder inside it."),
            (.Save(path: "/c.json", reason: "disk full"), "Couldn't save /c.json: disk full"),
            (.UnknownProject, "This Project is no longer in Bunode."),
            (.UnknownService, "This Service is no longer in Bunode."),
            (.EmptyCommand, "Enter a command."),
            (.UnmatchedQuote, "The command has an unmatched quote."),
            (.InvalidPort, "Use a port from 1 to 65535."),
            (.EmptyName, "Enter a name."),
            (.NameTooLong(max: 40), "Use at most 40 characters."),
            (.NameTaken(project: "acme"), "Name already used in acme"),
            (.NotExecutable(path: "/n"), "/n is not an executable file."),
            (.InvalidForceStopAfter(choices: [2, 5, 10]), "Force stop after must be one of [2, 5, 10] seconds."),
            (.InvalidLogLinesKept(choices: [1000, 5000]), "Log lines kept must be one of [1000, 5000]."),
            (.NoRuns, "This Service has no Runs yet."),
            (.ExportFailed(path: "/l.log", reason: "denied"), "Couldn't export to /l.log: denied"),
            (.ProcessExited(pid: 48121), "PID 48121 has already exited."),
            (.SignalFailed(pid: 48121, reason: "denied"), "Couldn't send SIGTERM to PID 48121: denied"),
        ]
        for (error, text) in cases {
            #expect(error.message == text)
            #expect((error as Error).userMessage == text)
        }
    }

    @Test func runNotesAreWorded() {
        let cases: [(RunNote, String)] = [
            (.exited, "Exited"),
            (.exitedWithCode(code: 1), "Exited with code 1"),
            (.exitedWithCode(code: 1000), "Exited with code 1000"),
            (.endedBySignal(signal: "SIGKILL"), "Ended by SIGKILL"),
            (.unmatchedQuote, "The command has an unmatched quote."),
            (.emptyCommand, "The command is empty."),
            (.folderMissing(path: "/app"), "/app is missing."),
            (.notInPath(program: "vite"), "vite wasn't found in PATH."),
            (.programMissing(program: "./dev"), "./dev doesn't exist."),
            (.couldNotStart(program: "x", reason: "denied"), "Couldn't start x: denied"),
            (.runnerFailed(reason: "denied"), "Couldn't start bunode-runner: denied"),
            (.runnerExited(status: "signal: 9"), "bunode-runner exited unexpectedly (signal: 9)."),
            (.runnerLost(reason: "gone"), "Lost track of bunode-runner: gone"),
            (.unknownEnd, "The Run ended for an unknown reason."),
        ]
        for (note, text) in cases {
            #expect(note.message == text)
        }
    }

    @Test func environmentProblemsAreWorded() {
        let wrap: (String) -> String = { "Couldn't read the environment (\($0)). Runs get Bunode's own." }
        let cases: [(EnvironmentProblem, String)] = [
            (.shellFailed(shell: "/bin/zsh", reason: "denied"), wrap("couldn't start /bin/zsh: denied")),
            (.noOutput(shell: "/bin/zsh"), wrap("/bin/zsh printed no environment")),
            (.timedOut(shell: "/bin/zsh", seconds: 10), wrap("/bin/zsh took longer than 10 s")),
            (.readFailed(reason: "broken pipe"), wrap("broken pipe")),
        ]
        for (problem, text) in cases {
            #expect(problem.message == text)
        }
    }

    @Test func configIssuesAreWorded() {
        let moved = ConfigIssue(path: "/c.json", backup: "/c.bad.json", problem: .unreadable(detail: "EOF"))
        #expect(moved.message == "/c.json couldn't be read (EOF). It was moved to /c.bad.json and Bunode started with no Projects.")
        let kept = ConfigIssue(path: "/c.json", backup: nil, problem: .newerFormat(version: 9))
        #expect(kept.message == "/c.json was written by a newer Bunode (format 9). Bunode started with no Projects.")
        let newerMoved = ConfigIssue(path: "/c.json", backup: "/c.bad.json", problem: .newerFormat(version: 9))
        #expect(newerMoved.message == "/c.json was written by a newer Bunode (format 9). It was moved to /c.bad.json and Bunode started with no Projects.")
        let unreadKept = ConfigIssue(path: "/c.json", backup: nil, problem: .unreadable(detail: "EOF"))
        #expect(unreadKept.message == "/c.json couldn't be read (EOF). Bunode started with no Projects.")
    }

    @Test func unknownSourcesAreNamed() {
        #expect(sourceName("Terminal") == "Terminal")
        #expect(sourceName(nil) == "Unknown")
    }
}
