//! Turns a Run's raw output into plain text plus style runs. Colors and
//! text attributes (SGR) are kept; every other escape sequence (cursor
//! movement, erasing, titles) is dropped.

use vte::{Params, Parser, Perform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogColor {
    /// The 256-color palette: 0–7 standard, 8–15 bright, 16–255 extended.
    Palette {
        index: u8,
    },
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

impl LogColor {
    fn palette(index: u8) -> LogColor {
        LogColor::Palette { index }
    }

    fn rgb(red: u8, green: u8, blue: u8) -> LogColor {
        LogColor::Rgb { red, green, blue }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogStyle {
    pub foreground: Option<LogColor>,
    pub background: Option<LogColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A styled stretch of a line, in UTF-16 code units (what AppKit counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogSpan {
    pub start: u32,
    pub length: u32,
    pub style: LogStyle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyledLine {
    pub text: String,
    /// Only stretches with a non-default style.
    pub spans: Vec<LogSpan>,
}

/// Parses one output stream line by line. Styles carry over from line to
/// line, as they do in a terminal.
#[derive(Default)]
pub struct LineParser {
    parser: Parser,
    state: State,
}

#[derive(Default)]
struct State {
    style: LogStyle,
    line: StyledLine,
    utf16_len: u32,
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
    pub fn line(&mut self, bytes: &[u8]) -> StyledLine {
        self.parser.advance(&mut self.state, bytes);
        self.state.utf16_len = 0;
        self.state.columns = 0;
        std::mem::take(&mut self.state.line)
    }
}

impl State {
    fn push(&mut self, c: char) {
        let width = c.len_utf16() as u32;
        self.line.text.push(c);
        if self.style != LogStyle::default() {
            match self.line.spans.last_mut() {
                Some(span)
                    if span.style == self.style && span.start + span.length == self.utf16_len =>
                {
                    span.length += width;
                }
                _ => self.line.spans.push(LogSpan {
                    start: self.utf16_len,
                    length: width,
                    style: self.style,
                }),
            }
        }
        self.utf16_len += width;
        self.columns += 1;
    }

    fn sgr(&mut self, params: &Params) {
        let mut values = params.iter().peekable();
        if values.peek().is_none() {
            self.style = LogStyle::default();
            return;
        }
        while let Some(param) = values.next() {
            let code = param.first().copied().unwrap_or(0);
            match code {
                0 => self.style = LogStyle::default(),
                1 => self.style.bold = true,
                2 => self.style.dim = true,
                3 => self.style.italic = true,
                4 => self.style.underline = true,
                22 => {
                    self.style.bold = false;
                    self.style.dim = false;
                }
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                30..=37 => self.style.foreground = Some(LogColor::palette((code - 30) as u8)),
                39 => self.style.foreground = None,
                40..=47 => self.style.background = Some(LogColor::palette((code - 40) as u8)),
                49 => self.style.background = None,
                90..=97 => self.style.foreground = Some(LogColor::palette((code - 90 + 8) as u8)),
                100..=107 => {
                    self.style.background = Some(LogColor::palette((code - 100 + 8) as u8))
                }
                38 | 48 => {
                    // Either `38;5;n` / `38;2;r;g;b`, or the colon forms in one param.
                    let color = if param.len() > 1 {
                        extended_color(&param[1..])
                    } else {
                        let mut rest: Vec<u16> = Vec::new();
                        if let Some(kind) = values.next() {
                            rest.push(kind.first().copied().unwrap_or(0));
                            let needed = if rest[0] == 2 { 3 } else { 1 };
                            for _ in 0..needed {
                                if let Some(v) = values.next() {
                                    rest.push(v.first().copied().unwrap_or(0));
                                }
                            }
                        }
                        extended_color(&rest)
                    };
                    if code == 38 {
                        self.style.foreground = color;
                    } else {
                        self.style.background = color;
                    }
                }
                _ => {}
            }
        }
    }
}

/// `[5, n]` or `[2, r, g, b]` (also `[2, colorspace, r, g, b]`).
fn extended_color(values: &[u16]) -> Option<LogColor> {
    let byte = |v: u16| v.min(255) as u8;
    match values {
        [5, n, ..] => Some(LogColor::palette(byte(*n))),
        [2, _, r, g, b] => Some(LogColor::rgb(byte(*r), byte(*g), byte(*b))),
        [2, r, g, b, ..] => Some(LogColor::rgb(byte(*r), byte(*g), byte(*b))),
        _ => None,
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
                self.line = StyledLine::default();
                self.utf16_len = 0;
                self.columns = 0;
            }
            // Backspace
            0x08 => {
                if let Some(c) = self.line.text.pop() {
                    self.utf16_len -= c.len_utf16() as u32;
                    self.columns = self.columns.saturating_sub(1);
                    let len = self.utf16_len;
                    self.line.spans.retain_mut(|span| {
                        if span.start >= len {
                            return false;
                        }
                        span.length = span.length.min(len - span.start);
                        true
                    });
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if action == 'm' {
            self.sgr(params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> StyledLine {
        LineParser::new().line(input.as_bytes())
    }

    #[test]
    fn plain_text_has_no_spans() {
        let line = parse("ready in 312 ms");
        assert_eq!(line.text, "ready in 312 ms");
        assert!(line.spans.is_empty());
    }

    #[test]
    fn colors_become_spans() {
        let line = parse("\x1b[32m✓\x1b[0m built \x1b[1;31merror\x1b[22;39m done");
        assert_eq!(line.text, "✓ built error done");
        assert_eq!(
            line.spans,
            [
                LogSpan {
                    start: 0,
                    length: 1,
                    style: LogStyle {
                        foreground: Some(LogColor::palette(2)),
                        ..LogStyle::default()
                    }
                },
                LogSpan {
                    start: 8,
                    length: 5,
                    style: LogStyle {
                        foreground: Some(LogColor::palette(1)),
                        bold: true,
                        ..LogStyle::default()
                    }
                },
            ]
        );
    }

    #[test]
    fn extended_and_bright_colors() {
        let fg = |s: &str| parse(s).spans[0].style.foreground;
        assert_eq!(fg("\x1b[38;5;208mx"), Some(LogColor::palette(208)));
        assert_eq!(fg("\x1b[38;2;10;20;30mx"), Some(LogColor::rgb(10, 20, 30)));
        assert_eq!(fg("\x1b[38:2::10:20:30mx"), Some(LogColor::rgb(10, 20, 30)));
        assert_eq!(fg("\x1b[94mx"), Some(LogColor::palette(12)));
    }

    #[test]
    fn other_sequences_are_dropped() {
        let line = parse("\x1b[2K\x1b[1G\x1b]0;title\x07Compiling…");
        assert_eq!(line.text, "Compiling…");
    }

    #[test]
    fn carriage_returns_keep_the_last_redraw() {
        assert_eq!(parse("10%\r50%\r100% done").text, "100% done");
    }

    #[test]
    fn tabs_become_spaces_to_the_next_stop() {
        assert_eq!(parse("a\tb\tc").text, "a   b   c");
        assert_eq!(parse("\tx").text, "    x");
    }

    #[test]
    fn style_carries_over_to_the_next_line() {
        let mut parser = LineParser::new();
        parser.line(b"\x1b[33mwarning: one");
        let next = parser.line(b"two\x1b[0m");
        assert_eq!(next.spans[0].style.foreground, Some(LogColor::palette(3)));
    }

    #[test]
    fn spans_count_utf16_units() {
        let line = parse("😀 \x1b[31mred");
        assert_eq!(line.spans[0].start, 3);
        assert_eq!(line.spans[0].length, 3);
    }
}
