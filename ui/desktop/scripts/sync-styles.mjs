import { cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const source = resolve(scriptDir, "..", "..", "..", "styles", "base.css");
const destinationDir = resolve(scriptDir, "..", "web", "styles");
const destination = resolve(destinationDir, "base.css");

if (!existsSync(source)) {
  console.error(`Global styles file not found: ${source}`);
  process.exit(1);
}

mkdirSync(destinationDir, { recursive: true });
cpSync(source, destination, { force: true });
console.log(`Synchronized styles: ${source} -> ${destination}`);
