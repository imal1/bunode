use std::io;

/// What can go wrong when the UI asks the Supervisor to do something.
/// Each variant carries its facts, not a sentence: the app words it in the
/// user's language (requirements §4.1). `Display` is English, for logs and
/// tests only.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path} is not a folder.")]
    NotAFolder { path: String },
    #[error("{path} doesn't exist.")]
    Missing { path: String },
    #[error("{path} is already a Project.")]
    AlreadyAProject { path: String },
    #[error("Choose the Project's folder or a folder inside it.")]
    OutsideProject,
    #[error("Couldn't save {path}: {source}")]
    Save {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("This Project is no longer in Bunode.")]
    UnknownProject,
    #[error("This Service is no longer in Bunode.")]
    UnknownService,
    #[error("Enter a command.")]
    EmptyCommand,
    #[error("The command has an unmatched quote.")]
    UnmatchedQuote,
    #[error("Use a port from 1 to 65535.")]
    InvalidPort,
    #[error("Enter a name.")]
    EmptyName,
    #[error("Use at most {max} characters.")]
    NameTooLong { max: u32 },
    #[error("Name already used in {project}")]
    NameTaken { project: String },
    #[error("{path} is not an executable file.")]
    NotExecutable { path: String },
    #[error("Force stop after must be one of {choices:?} seconds.")]
    InvalidForceStopAfter { choices: Vec<u32> },
    #[error("PID {pid} has already exited.")]
    ProcessExited { pid: u32 },
    #[error("Couldn't send SIGTERM to PID {pid}: {reason}")]
    SignalFailed { pid: u32, reason: String },
    /// An Agent Server's process group outlived SIGKILL.
    #[error("Couldn’t stop {name}.")]
    AgentServerSurvived { name: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
