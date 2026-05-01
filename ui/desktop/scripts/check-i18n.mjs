import { readFileSync, readdirSync } from "node:fs";

const baseDir = new URL("../web/i18n/", import.meta.url);
const enPath = new URL("en/common.json", baseDir);
const ruPath = new URL("ru/common.json", baseDir);

const en = JSON.parse(readFileSync(enPath, "utf8"));
const ru = JSON.parse(readFileSync(ruPath, "utf8"));

const missingInRu = Object.keys(en).filter((key) => !(key in ru));
const missingInEn = Object.keys(ru).filter((key) => !(key in en));
const mojibakePattern = /(?:\u0420.|\u0421.){2,}/u;
const placeholderPattern = /\?{4,}/;
const mojibakeInRu = Object.entries(ru)
  .filter(([, value]) => typeof value === "string" && mojibakePattern.test(value))
  .map(([key]) => key);
const placeholderLeaksInRu = Object.entries(ru)
  .filter(([, value]) => typeof value === "string" && placeholderPattern.test(value))
  .map(([key]) => key);

if (missingInRu.length || missingInEn.length || mojibakeInRu.length || placeholderLeaksInRu.length) {
  console.error("i18n check failed");
  if (missingInRu.length) {
    console.error("Missing in ru:", missingInRu.join(", "));
  }
  if (missingInEn.length) {
    console.error("Missing in en:", missingInEn.join(", "));
  }
  if (mojibakeInRu.length) {
    console.error("Broken ru encoding in keys:", mojibakeInRu.join(", "));
  }
  if (placeholderLeaksInRu.length) {
    console.error("Placeholder/question-mark leaks in ru keys:", placeholderLeaksInRu.join(", "));
  }
  process.exit(1);
}

const featureDirs = readdirSync(new URL("../web/features/", import.meta.url), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

if (!featureDirs.length) {
  console.error("No feature modules found.");
  process.exit(1);
}

console.log("i18n check passed. Feature modules:", featureDirs.join(", "));
