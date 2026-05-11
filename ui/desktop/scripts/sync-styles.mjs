import { cpSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const sourceDir = resolve(scriptDir, "..", "..", "..", "styles");
const source = resolve(sourceDir, "base.css");
const destinationDir = resolve(scriptDir, "..", "web", "styles");

if (!existsSync(source)) {
  console.error(`Global styles file not found: ${source}`);
  process.exit(1);
}

mkdirSync(destinationDir, { recursive: true });
const styleFiles = readdirSync(sourceDir).filter((name) => /^base(\..+)?\.css$/.test(name));
for (const fileName of styleFiles) {
  cpSync(resolve(sourceDir, fileName), resolve(destinationDir, fileName), { force: true });
}
console.log(`Synchronized ${styleFiles.length} base style layers from ${sourceDir} -> ${destinationDir}`);
