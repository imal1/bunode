//! A line of a Run's log comes from one of these (requirements §3.4).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
    /// A line Bunode itself wrote: the Run started, exited, couldn't start.
    System,
}
