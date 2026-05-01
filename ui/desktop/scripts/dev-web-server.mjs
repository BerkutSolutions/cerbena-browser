import { createServer } from "node:http";
import { get as httpGet } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { createReadStream } from "node:fs";
import { extname, join, normalize } from "node:path";

const PORT = Number(process.env.BROWSER_UI_DEV_PORT || 1430);
const ROOT = normalize(join(process.cwd(), "web"));

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".ico": "image/x-icon",
  ".svg": "image/svg+xml"
};

let keepAliveTimer = null;

function safePath(urlPath) {
  const base = urlPath.split("?")[0] || "/";
  const rel = base === "/" ? "/index.html" : base;
  const full = normalize(join(ROOT, rel));
  if (!full.startsWith(ROOT)) return null;
  return full;
}

const server = createServer(async (req, res) => {
  try {
    const filePath = safePath(req.url || "/");
    if (!filePath) {
      res.writeHead(403);
      res.end("forbidden");
      return;
    }

    await stat(filePath);
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Content-Type", MIME[extname(filePath)] || "application/octet-stream");
    createReadStream(filePath).pipe(res);
  } catch {
    try {
      const fallback = join(ROOT, "index.html");
      const html = await readFile(fallback);
      res.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": "text/html; charset=utf-8"
      });
      res.end(html);
    } catch {
      res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
      res.end("asset not found: index.html");
    }
  }
});

function holdProcessOpen() {
  if (!keepAliveTimer) {
    keepAliveTimer = setInterval(() => {}, 60_000);
  }
}

function probeExistingServer() {
  return new Promise((resolve) => {
    const req = httpGet(`http://127.0.0.1:${PORT}/`, (res) => {
      res.resume();
      resolve(res.statusCode && res.statusCode >= 200 && res.statusCode < 500);
    });
    req.on("error", () => resolve(false));
    req.setTimeout(2_000, () => {
      req.destroy();
      resolve(false);
    });
  });
}

server.on("error", async (error) => {
  if (error?.code === "EADDRINUSE") {
    const existingOk = await probeExistingServer();
    if (existingOk) {
      console.log(`[dev-web] reusing existing server at http://127.0.0.1:${PORT}`);
      holdProcessOpen();
      return;
    }
  }

  console.error(error);
  process.exit(1);
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`[dev-web] serving ${ROOT} at http://127.0.0.1:${PORT}`);
  holdProcessOpen();
});
