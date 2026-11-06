// Prints as fast as it can: `node flood.js [lines]` (default: forever).
// Every 10th line goes to stderr and every 7th is colored.
const total = process.argv[2] === undefined ? Infinity : Number(process.argv[2]);
let i = 0;

function write() {
  while (i < total) {
    i += 1;
    const text = i % 7 === 0 ? `\x1b[32mline ${i}\x1b[0m` : `line ${i}`;
    const stream = i % 10 === 0 ? process.stderr : process.stdout;
    if (!stream.write(`${text}\n`)) {
      stream.once("drain", write);
      return;
    }
  }
}
write();
