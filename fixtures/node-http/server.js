// Node HTTP server for tests: listens on $PORT (0 = any free port).
const http = require("node:http");

const server = http.createServer((_request, response) => response.end("ok\n"));
server.listen(Number(process.env.PORT ?? 0), "127.0.0.1", () => {
  console.log(`listening on ${server.address().port}`);
});
