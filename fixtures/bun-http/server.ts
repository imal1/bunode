// Bun HTTP server for tests: listens on $PORT (0 = any free port).
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(process.env.PORT ?? 0),
  fetch: () => new Response("ok\n"),
});
console.log(`listening on ${server.port}`);
