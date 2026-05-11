#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { resolve } from "node:path";

const repoRoot = execSync("git rev-parse --show-toplevel", { cwd: process.cwd() })
  .toString("utf8")
  .trim();
const configPath = resolve(repoRoot, "scripts", "text-integrity.config.json");
const config = JSON.parse(readFileSync(configPath, "utf8"));

const binaryExtensions = new Set(config.binary_extensions ?? []);
const excludedPrefixes = config.exclude_prefixes ?? [];
const includedRoots = config.text_roots ?? [];
const suspiciousMojibakePatterns = (config.mojibake_patterns ?? []).map((value) => new RegExp(value, "g"));
const mojibakeAllowlistPatterns = (config.mojibake_allowlist_patterns ?? []).map((value) => new RegExp(value));
const selfPatternConfigPath = "scripts/text-integrity.config.json";

function isWithinIncludedRoots(path) {
  return includedRoots.some((root) => path === root || path.startsWith(root));
}

function isLikelyTextPath(path) {
  if (excludedPrefixes.some((prefix) => path.startsWith(prefix))) return false;
  if (!isWithinIncludedRoots(path)) return false;
  const lower = path.toLowerCase();
  for (const ext of binaryExtensions) {
    if (lower.endsWith(ext)) return false;
  }
  return true;
}

function listTrackedFiles() {
  const output = execSync("git ls-files -z", { cwd: repoRoot });
  return output
    .toString("utf8")
    .split("\u0000")
    .filter(Boolean)
    .filter(isLikelyTextPath);
}

function validateFile(path) {
  const normalizedPath = path.replaceAll("\\", "/");
  const absolute = resolve(repoRoot, path);
  let buffer;
  try {
    buffer = readFileSync(absolute);
  } catch (error) {
    if (error?.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  const utf8 = buffer.toString("utf8");
  const problems = [];
  if (utf8.includes("\uFFFD")) {
    problems.push("contains UTF-8 replacement character (invalid UTF-8 bytes likely present)");
  }
  if (utf8.includes("\u0000")) {
    problems.push("contains NUL byte (binary payload likely committed as text)");
  }
  const allowMojibake =
    normalizedPath === selfPatternConfigPath ||
    mojibakeAllowlistPatterns.some((pattern) => pattern.test(normalizedPath));
  if (!allowMojibake) {
    for (const pattern of suspiciousMojibakePatterns) {
      if (pattern.test(utf8)) {
        problems.push(`contains suspicious mojibake pattern: ${pattern}`);
        break;
      }
    }
  }
  return problems;
}

const failures = [];
for (const file of listTrackedFiles()) {
  const problems = validateFile(file);
  for (const problem of problems) {
    failures.push({ file, problem });
  }
}

if (failures.length) {
  console.error("Text integrity check failed:");
  for (const failure of failures) {
    console.error(`- ${failure.file}: ${failure.problem}`);
  }
  process.exit(1);
}

console.log("Text integrity check passed");
