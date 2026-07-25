#!/usr/bin/env node
// V12 — Secret scan (cross-platform, Node built-ins only)
// Usage: node scripts/verify-no-secrets.mjs
import { readFileSync, readdirSync, statSync, existsSync } from "fs";
import { join, relative, extname } from "path";
import { fileURLToPath } from "url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const PASS = "\x1b[32m✓\x1b[0m";
const FAIL = "\x1b[31m✗\x1b[0m";
const WARN = "\x1b[33m⚠\x1b[0m";

const SKIP_DIRS = new Set([
  "node_modules", "target", ".git", "vendor",
  "chimera-refs", "dist", ".cache",
]);
const TEXT_EXTS = new Set([
  ".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs",
  ".toml", ".json", ".yaml", ".yml", ".md",
  ".ps1", ".sh", ".txt", ".env", ".cfg", ".conf",
]);

// Patterns that indicate a real secret
const SECRET_PATTERNS = [
  { name: "OpenAI API key",     re: /\bsk-[A-Za-z0-9]{20,}\b/ },
  { name: "Bearer token",       re: /Bearer\s+[A-Za-z0-9+/]{20,}/ },
  { name: "Private key header", re: /-----BEGIN\s+(?:RSA\s+|EC\s+)?PRIVATE KEY-----/ },
  { name: "URL with credentials", re: /https?:\/\/[^@\s]{3,}:[^@\s]{3,}@/ },
  { name: "AWS key",            re: /\bAKIA[0-9A-Z]{16}\b/ },
  { name: ".env secret assignment", re: /^(?:API_KEY|SECRET|TOKEN|PASSWORD)\s*=\s*.{8,}/m },
];

let failures = 0;
let warnings = 0;
const findings = [];

function scanFile(filePath) {
  const ext = extname(filePath).toLowerCase();
  if (!TEXT_EXTS.has(ext) && ext !== "") return;
  let content;
  try {
    content = readFileSync(filePath, "utf8");
  } catch {
    return; // binary or unreadable
  }
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    for (const { name, re } of SECRET_PATTERNS) {
      if (re.test(line)) {
        // Allowlist: variable names in test fixtures that use placeholder values
        const trimmed = line.trim();
        if (
          trimmed.startsWith("//") || trimmed.startsWith("#") ||
          trimmed.startsWith("*") ||
          line.includes("YOUR_API_KEY") || line.includes("PLACEHOLDER") ||
          line.includes("sk-test-") || line.includes("example") ||
          line.includes("<<") || line.includes("{{")
        ) continue;
        findings.push({ file: relative(ROOT, filePath), line: i + 1, pattern: name });
        failures++;
      }
    }
  }
}

function walk(dir) {
  if (!existsSync(dir)) return;
  for (const entry of readdirSync(dir)) {
    if (SKIP_DIRS.has(entry)) continue;
    const full = join(dir, entry);
    try {
      const st = statSync(full);
      if (st.isDirectory()) walk(full);
      else if (st.isFile()) scanFile(full);
    } catch { /* skip */ }
  }
}

walk(ROOT);

if (findings.length === 0) {
  console.log(`${PASS} verify-no-secrets: no secret patterns found`);
} else {
  console.log(`${FAIL} verify-no-secrets: ${findings.length} finding(s)\n`);
  for (const f of findings) {
    console.log(`  ${FAIL} ${f.file}:${f.line}  [${f.pattern}]`);
  }
}

process.exit(failures > 0 ? 1 : 0);
