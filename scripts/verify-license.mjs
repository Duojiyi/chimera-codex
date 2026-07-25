#!/usr/bin/env node
// V11 — License compliance check (cross-platform, Node built-ins only)
// Usage: node scripts/verify-license.mjs
import { readFileSync, existsSync, readdirSync, statSync } from "fs";
import { join, relative } from "path";
import { fileURLToPath } from "url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const PASS = "\x1b[32m✓\x1b[0m";
const FAIL = "\x1b[31m✗\x1b[0m";
let failures = 0;

function check(label, ok, detail = "") {
  console.log(`${ok ? PASS : FAIL} ${label}${detail ? `: ${detail}` : ""}`);
  if (!ok) failures++;
}

// 1. Workspace Cargo.toml declares AGPL-3.0-only
const workspaceToml = readFileSync(join(ROOT, "Cargo.toml"), "utf8");
check(
  "Workspace Cargo.toml license = AGPL-3.0-only",
  workspaceToml.includes('license = "AGPL-3.0-only"'),
);

// 2. All crate Cargo.toml files either declare the workspace license or AGPL-3.0-only
function findCargoTomls(dir, results = []) {
  if (!existsSync(dir)) return results;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory() && entry !== "target" && entry !== ".git") {
      findCargoTomls(full, results);
    } else if (entry === "Cargo.toml" && full !== join(ROOT, "Cargo.toml")) {
      results.push(full);
    }
  }
  return results;
}

const crateTomlPaths = findCargoTomls(join(ROOT, "crates")).concat(
  findCargoTomls(join(ROOT, "apps")),
);
let badLicense = [];
for (const p of crateTomlPaths) {
  const content = readFileSync(p, "utf8");
  const hasWorkspace = content.includes("license.workspace");
  const hasExplicit = content.includes('"AGPL-3.0-only"');
  if (!hasWorkspace && !hasExplicit) {
    badLicense.push(relative(ROOT, p));
  }
}
check(
  "All crate Cargo.toml files inherit or declare AGPL-3.0-only",
  badLicense.length === 0,
  badLicense.length > 0 ? badLicense.join(", ") : "",
);

// 3. THIRD_PARTY_SOURCES.md exists and has at least 4 registered sources
const tpPath = join(ROOT, "THIRD_PARTY_SOURCES.md");
check("THIRD_PARTY_SOURCES.md exists", existsSync(tpPath));
if (existsSync(tpPath)) {
  const tpContent = readFileSync(tpPath, "utf8");
  const sourceCount = (tpContent.match(/^##\s+\d+\./gm) || []).length;
  check(
    "THIRD_PARTY_SOURCES.md has ≥4 registered sources",
    sourceCount >= 4,
    `found ${sourceCount}`,
  );
}

// 4. NOTICE file exists and is non-empty
const noticePath = join(ROOT, "NOTICE");
check("NOTICE file exists", existsSync(noticePath));
if (existsSync(noticePath)) {
  const noticeContent = readFileSync(noticePath, "utf8").trim();
  check("NOTICE file is non-empty", noticeContent.length > 20);
}

// 5. LICENSE file exists and contains AGPL
const licensePath = join(ROOT, "LICENSE");
check("LICENSE file exists", existsSync(licensePath));
if (existsSync(licensePath)) {
  const licContent = readFileSync(licensePath, "utf8");
  check(
    "LICENSE contains GNU AGPL text",
    licContent.includes("GNU AFFERO GENERAL PUBLIC LICENSE") ||
      licContent.includes("GNU General Public License"),
  );
}

console.log(`\n${failures === 0 ? PASS : FAIL} verify-license: ${failures === 0 ? "PASS" : `${failures} failure(s)`}`);
process.exit(failures > 0 ? 1 : 0);
