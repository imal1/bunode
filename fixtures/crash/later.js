// Runs for a moment, then throws.
console.log("running");
setTimeout(() => {
  throw new Error("crashing on purpose");
}, 500);
