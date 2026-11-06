use std::io;

/// What can go wrong when the UI asks the Supervisor to do something. The
/// messages are written for the user.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path} is not a folder.")]
    NotAFolder { path: String },
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
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
