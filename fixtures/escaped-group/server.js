// The server launcher.js starts outside the Run's process group.
const fs = require("node:fs");
const http = require("node:http");

const [pidFile, ...flags] = process.argv.slice(2);
if (flags.includes("--ignore-sigterm")) {
  process.on("SIGTERM", () => {});
}
const server = http.createServer((_request, response) => response.end("ok\n"));
server.listen(0, "127.0.0.1", () => {
  fs.appendFileSync(pidFile, `${process.pid}\n`);
  console.log(`listening on ${server.address().port}`);
});
