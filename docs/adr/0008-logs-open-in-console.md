# Logs open in Console, not in the popover

The popover had a Logs page: a virtualized AppKit table of the Run's last ~10,000 lines with ANSI colors, search, Clear, Copy and Export. At 340 pt wide and 400 pt tall it showed too little of a busy dev server, and making it bigger would mean a second window (ADR-0004). Every Run's full output is already written to a plain-text file, so Bunode now hands that file to macOS Console, which follows it as it grows and brings its own resizable window, search and copy. The Service view keeps a Logs row that opens it; there is no Logs page.

## Considered Options

- **Keep the popover page and add an Open in Console button**: tried first; once Console was there, the popover page wasn't used.
- **A resizable log window of Bunode's own**: rejected for now; it is the most work, needs an exception to ADR-0004, and duplicates what Console does.
- **`tail -f` in Terminal**: rejected; it doesn't follow a Run into its next file and offers no search.

## Consequences

The log file has to keep up: the writer flushes whenever it has caught up, so Console is live. Console stays on the file it opened, so a Run's file is allowed to grow to 50 MB before the next one starts. Console shows no ANSI colors; stdout and stderr are told apart by the `out` / `err` tag on each line. The in-memory log, its search, the ANSI styling and Advanced › Log lines kept went with the page: output goes only to the file, colors stripped. Supersedes the "logs inside the popover" part of ADR-0004.
