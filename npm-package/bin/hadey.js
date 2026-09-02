#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");

const executable = path.join(__dirname, "..", "binaries", "hadey.exe");

const child = spawn(executable, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false
});

child.on("error", (error) => {
  console.error("Failed to start Hadey:", error.message);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 0);
  }
});