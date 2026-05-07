import { execFileSync } from "node:child_process";
import { platform } from "node:os";

const PORT = process.env.BROWSER_UI_DEV_PORT || "1430";

function run(command, args) {
  return execFileSync(command, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
}

function stopWindows() {
  try {
    const script = [
      "$port = " + PORT,
      "$connections = Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $port -ErrorAction SilentlyContinue",
      "if (-not $connections) { Write-Output \"NONE\"; exit 0 }",
      "$pids = $connections | Select-Object -ExpandProperty OwningProcess -Unique",
      "$pids | ForEach-Object { $_ }",
    ].join("; ");
    const output = run("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]);
    const pids = output
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => /^\d+$/.test(line));

    if (!pids.length) {
      console.log(`No process is listening on port ${PORT}`);
      return;
    }

    for (const pid of pids) {
      console.log(`Stopping PID ${pid} on port ${PORT}`);
      run("taskkill", ["/PID", pid, "/F"]);
    }
  } catch (error) {
    console.error("Failed to inspect TCP listeners on Windows:", error.message);
    process.exit(1);
  }
}

function stopUnix() {
  let output = "";
  try {
    output = run("sh", ["-lc", `lsof -ti tcp:${PORT} || true`]);
  } catch (error) {
    console.error("Failed to inspect TCP listeners on Unix:", error.message);
    process.exit(1);
  }

  const pids = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^\d+$/.test(line));

  if (!pids.length) {
    console.log(`No process is listening on port ${PORT}`);
    return;
  }

  for (const pid of pids) {
    console.log(`Stopping PID ${pid} on port ${PORT}`);
    run("kill", ["-9", pid]);
  }
}

if (platform() === "win32") {
  stopWindows();
} else {
  stopUnix();
}

console.log(`Dev web server port ${PORT} is free.`);
