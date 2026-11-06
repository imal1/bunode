// Listens on two ports at once: $PORT (0 = any free port) and any free one.
const http = require("node:http");

const handler = (_request, response) => response.end("ok\n");
const first = http.createServer(handler);
const second = http.createServer(handler);
first.listen(Number(process.env.PORT ?? 0), "127.0.0.1", () => {
  second.listen(0, "127.0.0.1", () => {
    console.log(`listening on ${first.address().port} and ${second.address().port}`);
  });
});
