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
    /// With a search: matches are numbered from `match_first`, and there
    /// are `match_count` of them. Without one, both are 0.
    pub match_first: u64,
    pub match_count: u64,
    /// Changes whenever the search changes, so the UI starts over.
    pub filter_generation: u64,
}

/// The lines matching a search, kept up to date as lines arrive and leave.
struct Filter {
    /// Lowercased.
    query: String,
    /// The `seq` of each matching line, oldest first.
    matches: VecDeque<u64>,
    /// How many matches came before the first one still in `matches`.
    base: u64,
}

impl Filter {
    fn matches(&self, line: &LogLine) -> bool {
        line.text.to_lowercase().contains(&self.query)
    }
}

pub struct LogBuffer {
    lines: VecDeque<LogLine>,
    capacity: usize,
    next_seq: u64,
    dropped: u64,
    /// Lines before this one were cleared by the user.
    cleared_before: u64,
    filter: Option<Filter>,
    filter_generation: u64,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> LogBuffer {
        LogBuffer {
            lines: VecDeque::new(),
            capacity: capacity.max(1),
            next_seq: 0,
            dropped: 0,
            cleared_before: 0,
            filter: None,
            filter_generation: 0,
        }
    }

    pub fn push(&mut self, stream: LogStream, time_ms: i64, line: StyledLine) {
        if self.lines.len() == self.capacity
            && let Some(evicted) = self.lines.pop_front()
        {
            if evicted.seq >= self.cleared_before {
                self.dropped += 1;
            }
            if let Some(filter) = &mut self.filter
                && filter.matches.front() == Some(&evicted.seq)
            {
                filter.matches.pop_front();
                filter.base += 1;
            }
        }
        let line = LogLine {
            seq: self.next_seq,
            time_ms,
            stream,
            text: line.text,
            spans: line.spans,
        };
        if let Some(filter) = &mut self.filter
            && filter.matches(&line)
        {
            filter.matches.push_back(line.seq);
        }
        self.lines.push_back(line);
        self.next_seq += 1;
    }

    /// Shows only lines containing `query`, ignoring case; an empty query
    /// shows everything.
    pub fn set_filter(&mut self, query: &str) {
        let query = query.trim().to_lowercase();
        if self.filter.as_ref().map(|f| f.query.as_str()) == Some(query.as_str())
            || (query.is_empty() && self.filter.is_none())
        {
            return;
        }
        self.filter_generation += 1;
        if query.is_empty() {
            self.filter = None;
            return;
        }
        let mut filter = Filter {
            query,
            matches: VecDeque::new(),
            base: 0,
        };
        for line in self.lines.iter().filter(|l| l.seq >= self.cleared_before) {
            if filter.matches(line) {
                filter.matches.push_back(line.seq);
            }
        }
        self.filter = Some(filter);
    }

    pub fn filter_query(&self) -> Option<&str> {
        self.filter.as_ref().map(|f| f.query.as_str())
    }

    /// Up to `max` matching lines, starting at match number `from`.
    pub fn matching_lines(&self, from: u64, max: usize) -> Vec<LogLine> {
        let Some(filter) = &self.filter else {
            return Vec::new();
        };
        let Some(oldest) = self.lines.front().map(|l| l.seq) else {
            return Vec::new();
        };
        let skip = from.saturating_sub(filter.base) as usize;
        filter
            .matches
            .iter()
            .skip(skip)
            .take(max)
            .filter_map(|&seq| self.lines.get((seq - oldest) as usize).cloned())
            .collect()
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
        let (match_first, match_count) = self
            .filter
            .as_ref()
            .map_or((0, 0), |f| (f.base, f.matches.len() as u64));
        LogInfo {
            first,
            count: self.next_seq - first,
            dropped: self.dropped,
            match_first,
            match_count,
            filter_generation: self.filter_generation,
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
        if let Some(filter) = &mut self.filter {
            filter.base += filter.matches.len() as u64;
            filter.matches.clear();
        }
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
        let info = log.info();
        assert_eq!((info.first, info.count, info.dropped), (2, 3, 2));
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
        let info = log.info();
        assert_eq!((info.first, info.count, info.dropped), (5, 0, 0));
        log.push(LogStream::Stderr, 0, text("after"));
        assert_eq!(texts(&log.lines(0, 10)), ["after"]);
        assert_eq!(log.info().count, 1);
        // Evicting cleared lines is not a drop.
        log.push_system(0, "a");
        log.push_system(0, "b");
        assert_eq!(log.info().dropped, 0);
    }

    #[test]
    fn search_finds_lines_and_keeps_up() {
        let mut log = LogBuffer::new(4);
        for t in ["GET /", "error: boom", "GET /api", "Error again"] {
            log.push(LogStream::Stdout, 0, text(t));
        }
        log.set_filter("ERROR");
        let info = log.info();
        assert_eq!((info.match_first, info.match_count), (0, 2));
        assert_eq!(
            texts(&log.matching_lines(0, 10)),
            ["error: boom", "Error again"]
        );

        // New matches arrive; old ones leave with their lines.
        log.push(LogStream::Stderr, 0, text("another error"));
        log.push(LogStream::Stdout, 0, text("fine"));
        let info = log.info();
        assert_eq!((info.match_first, info.match_count), (1, 2));
        assert_eq!(
            texts(&log.matching_lines(1, 10)),
            ["Error again", "another error"]
        );
        assert_eq!(texts(&log.matching_lines(2, 10)), ["another error"]);

        let generation = info.filter_generation;
        log.set_filter("get");
        assert_ne!(log.info().filter_generation, generation);
        assert_eq!(texts(&log.matching_lines(0, 10)), ["GET /api"]);
        log.set_filter("");
        assert_eq!(log.info().match_count, 0);
        assert_eq!(log.filter_query(), None);
    }

    #[test]
    fn clear_empties_the_search_too() {
        let mut log = LogBuffer::new(10);
        log.push(LogStream::Stdout, 0, text("error 1"));
        log.set_filter("error");
        log.clear();
        assert_eq!(log.info().match_count, 0);
        log.push(LogStream::Stdout, 0, text("error 2"));
        let info = log.info();
        assert_eq!((info.match_first, info.match_count), (1, 1));
        assert_eq!(texts(&log.matching_lines(1, 10)), ["error 2"]);
    }
}
