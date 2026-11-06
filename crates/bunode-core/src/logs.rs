//! The bounded, in-memory log of a Run (requirements §2.2, §3.4). The UI is
//! told how many lines there are and fetches the ones it shows.

use std::collections::VecDeque;

use crate::ansi::{LogSpan, StyledLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
    /// A line Bunode itself wrote: the Run started, exited, couldn't start.
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Position in the Run's output, from 0, never reused.
    pub seq: u64,
    /// When it arrived, in milliseconds since 1970.
    pub time_ms: i64,
    pub stream: LogStream,
    pub text: String,
    pub spans: Vec<LogSpan>,
}

/// What the UI needs to know to show a Run's log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogInfo {
    /// The `seq` of the first line shown.
    pub first: u64,
    /// How many lines are shown, from `first` on.
    pub count: u64,
    /// Lines that no longer fit and were dropped since the last Clear.
    pub dropped: u64,
}

pub struct LogBuffer {
    lines: VecDeque<LogLine>,
    capacity: usize,
    next_seq: u64,
    dropped: u64,
    /// Lines before this one were cleared by the user.
    cleared_before: u64,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> LogBuffer {
        LogBuffer {
            lines: VecDeque::new(),
            capacity: capacity.max(1),
            next_seq: 0,
            dropped: 0,
            cleared_before: 0,
        }
    }

    pub fn push(&mut self, stream: LogStream, time_ms: i64, line: StyledLine) {
        if self.lines.len() == self.capacity
            && let Some(evicted) = self.lines.pop_front()
            && evicted.seq >= self.cleared_before
        {
            self.dropped += 1;
        }
        self.lines.push_back(LogLine {
            seq: self.next_seq,
            time_ms,
            stream,
            text: line.text,
            spans: line.spans,
        });
        self.next_seq += 1;
    }

    pub fn push_system(&mut self, time_ms: i64, text: impl Into<String>) {
        self.push(
            LogStream::System,
            time_ms,
            StyledLine {
                text: text.into(),
                spans: Vec::new(),
            },
        );
    }

    pub fn info(&self) -> LogInfo {
        let oldest = self.lines.front().map_or(self.next_seq, |l| l.seq);
        let first = oldest.max(self.cleared_before);
        LogInfo {
            first,
            count: self.next_seq - first,
            dropped: self.dropped,
        }
    }

    /// Up to `max` shown lines, starting at `from` (or the first one shown).
    pub fn lines(&self, from: u64, max: usize) -> Vec<LogLine> {
        let Some(oldest) = self.lines.front().map(|l| l.seq) else {
            return Vec::new();
        };
        let from = from.max(self.cleared_before).max(oldest);
        let skip = (from - oldest) as usize;
        self.lines.iter().skip(skip).take(max).cloned().collect()
    }

    /// Hides everything so far. Lines on disk are not touched.
    pub fn clear(&mut self) {
        self.cleared_before = self.next_seq;
        self.dropped = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> StyledLine {
        StyledLine {
            text: t.into(),
            spans: vec![],
        }
    }

    fn texts(lines: &[LogLine]) -> Vec<&str> {
        lines.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn keeps_at_most_capacity_lines_and_counts_drops() {
        let mut log = LogBuffer::new(3);
        for i in 0..5 {
            log.push(LogStream::Stdout, 0, text(&i.to_string()));
        }
        assert_eq!(
            log.info(),
            LogInfo {
                first: 2,
                count: 3,
                dropped: 2
            }
        );
        assert_eq!(texts(&log.lines(0, 10)), ["2", "3", "4"]);
        assert_eq!(texts(&log.lines(3, 1)), ["3"]);
        assert_eq!(texts(&log.lines(9, 10)), Vec::<&str>::new());
    }

    #[test]
    fn clear_hides_lines_and_resets_drops() {
        let mut log = LogBuffer::new(3);
        for i in 0..5 {
            log.push(LogStream::Stdout, 0, text(&i.to_string()));
        }
        log.clear();
        assert_eq!(
            log.info(),
            LogInfo {
                first: 5,
                count: 0,
                dropped: 0
            }
        );
        log.push(LogStream::Stderr, 0, text("after"));
        assert_eq!(texts(&log.lines(0, 10)), ["after"]);
        assert_eq!(log.info().count, 1);
        // Evicting cleared lines is not a drop.
        log.push_system(0, "a");
        log.push_system(0, "b");
        assert_eq!(log.info().dropped, 0);
    }
}
