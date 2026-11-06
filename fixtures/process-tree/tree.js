// Spawns a chain of `depth` descendants (default 2) and keeps them alive.
// Every process appends its PID to the file named by the second argument
// (or $TREE_PIDS).
const { spawn } = require("node:child_process");
const fs = require("node:fs");

const depth = Number(process.argv[2] ?? 2);
const pidFile = process.argv[3] ?? process.env.TREE_PIDS;
fs.appendFileSync(pidFile, `${process.pid}\n`);
if (depth > 0) {
  spawn(process.execPath, [__filename, String(depth - 1), pidFile], { stdio: "inherit" });
}
setInterval(() => {}, 1000);
