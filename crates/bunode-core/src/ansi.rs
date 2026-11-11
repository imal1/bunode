//! Turns a Run's raw output into plain text for its log file. Every escape
//! sequence (colors, cursor movement, erasing, titles) is dropped.

use vte::{Parser, Perform};

/// Parses one output stream line by line.
#[derive(Default)]
pub struct LineParser {
    parser: Parser,
    state: State,
}

#[derive(Default)]
struct State {
    line: String,
    /// Characters on the line so far, for tab stops.
    columns: u32,
}

/// Tabs become spaces up to the next multiple of this.
const TAB_WIDTH: u32 = 4;

impl LineParser {
    pub fn new() -> LineParser {
        LineParser::default()
    }

    /// Parses one line, without its line ending.
    pub fn line(&mut self, bytes: &[u8]) -> String {
        self.parser.advance(&mut self.state, bytes);
        self.state.columns = 0;
        std::mem::take(&mut self.state.line)
    }
}

impl State {
    fn push(&mut self, c: char) {
        self.line.push(c);
        self.columns += 1;
    }
}

impl Perform for State {
    fn print(&mut self, c: char) {
        self.push(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\t' => {
                let spaces = TAB_WIDTH - self.columns % TAB_WIDTH;
                for _ in 0..spaces {
                    self.push(' ');
                }
            }
            // A carriage return redraws the line, as progress bars do: keep
            // only what comes after it.
            b'\r' => {
                self.line.clear();
                self.columns = 0;
            }
            // Backspace
            0x08 if self.line.pop().is_some() => {
                self.columns = self.columns.saturating_sub(1);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> String {
        LineParser::new().line(input.as_bytes())
    }

    #[test]
    fn plain_text_stays() {
        assert_eq!(parse("ready in 312 ms"), "ready in 312 ms");
    }

    #[test]
    fn escape_sequences_are_dropped() {
        assert_eq!(
            parse("\x1b[32m✓\x1b[0m built \x1b[1;31merror\x1b[22;39m done"),
            "✓ built error done"
        );
        assert_eq!(parse("\x1b[38;2;10;20;30mx"), "x");
        assert_eq!(
            parse("\x1b[2K\x1b[1G\x1b]0;title\x07Compiling…"),
            "Compiling…"
        );
    }

    #[test]
    fn carriage_returns_keep_the_last_redraw() {
        assert_eq!(parse("10%\r50%\r100% done"), "100% done");
    }

    #[test]
    fn backspace_erases() {
        assert_eq!(parse("ab\x08c"), "ac");
    }

    #[test]
    fn tabs_become_spaces_to_the_next_stop() {
        assert_eq!(parse("a\tb\tc"), "a   b   c");
        assert_eq!(parse("\tx"), "    x");
    }

    #[test]
    fn a_sequence_split_across_lines_is_still_dropped() {
        let mut parser = LineParser::new();
        assert_eq!(parser.line(b"one\x1b[3"), "one");
        assert_eq!(parser.line(b"3mtwo"), "two");
    }
}
