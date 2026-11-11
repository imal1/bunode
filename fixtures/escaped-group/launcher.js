// Like pnpm 12 running a script: starts the server in a new process group
// (and session), so it leaves the Run's group, then waits for it. It does
// not pass signals on.
//
//   node launcher.js PID_FILE [--ignore-sigterm]
//
// Both processes append their PID to PID_FILE, launcher first. The server
// listens on a free port and prints "listening on PORT".
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const [pidFile, ...flags] = process.argv.slice(2);
fs.appendFileSync(pidFile, `${process.pid}\n`);
spawn(process.execPath, [path.join(__dirname, "server.js"), pidFile, ...flags], {
  stdio: "inherit",
  detached: true,
});
setInterval(() => {}, 1000);
