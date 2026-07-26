#!/usr/bin/env node
// V13 — Chimera++ v2 verification orchestrator (cross-platform, Node built-ins only)
// Usage: node scripts/verify-v2.mjs [--only V1,V7] [--skip V8]
import { spawnSync } from "child_process";
import { existsSync } from "fs";
import { join } from "path";
import { fileURLToPath } from "url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const IS_CI = process.env.CI === "true";
const c = IS_CI ? (_, s) => s : (code, s) => `\x1b[${code}m${s}\x1b[0m`;
const GREEN = (s) => c("32", s);
const RED   = (s) => c("31", s);
const CYAN  = (s) => c("36", s);
const DIM   = (s) => c("2",  s);

// Parse --only / --skip flags
const args = process.argv.slice(2);
const onlyArg  = args.find(a => a.startsWith("--only="))?.split("=")[1];
const skipArg  = args.find(a => a.startsWith("--skip="))?.split("=")[1];
const onlySet  = onlyArg  ? new Set(onlyArg.split(","))  : null;
const skipSet  = skipArg  ? new Set(skipArg.split(","))  : new Set();

// The v2 crates. V7 is scoped to these rather than `cargo test --workspace`:
// the 1.x suite is red on this branch by construction (codex-plus-manager's
// upstream_sync_* tests assert on sync-upstream.yml, which v2 deleted per ADR).
// Enumerating keeps the gate honest instead of hiding a real failure behind a
// blanket exclusion — and V15 check 1 separately proves no v2 crate depends on
// the 1.x tree, so nothing v2 ships is covered by those tests.
const V2_CRATES = [
  "chimera-domain",
  "chimera-platform",
  "chimera-provider",
  "chimera-runtime",
  "chimera-theme",
  "chimera-migration",
  "chimera-update",
  "mirror-contract",
];

const CHECKS = [
  {
    id: "V1",
    label: "Rust format",
    cmd: "cargo",
    args: ["fmt", "--all", "--", "--check"],
    cwd: ROOT,
    skipIfMissing: "Cargo.toml",
  },
  {
    id: "V7",
    label: `Rust v2 crate tests (${V2_CRATES.length} crates)`,
    cmd: "cargo",
    args: ["test", "--locked", ...V2_CRATES.flatMap(c => ["-p", c])],
    cwd: ROOT,
    skipIfMissing: "Cargo.toml",
  },
  {
    id: "V9",
    label: "Mirror contract",
    cmd: "node",
    args: ["scripts/test-mirror-contract.mjs"],
    cwd: ROOT,
  },
  {
    id: "V10",
    label: "Bundle contract",
    cmd: "node",
    args: ["scripts/test-bundle-contract.mjs"],
    cwd: ROOT,
  },
  {
    id: "V10r",
    label: "Release tooling self-tests",
    cmd: "node",
    args: ["scripts/v2-release-manifest.mjs", "--self-test"],
    cwd: ROOT,
  },
  {
    id: "V10s",
    label: "Manifest signing self-test",
    cmd: "node",
    args: ["scripts/sign-manifest.mjs", "--self-test"],
    cwd: ROOT,
  },
  {
    id: "V11",
    label: "License compliance",
    cmd: "node",
    args: ["scripts/verify-license.mjs"],
    cwd: ROOT,
  },
  {
    id: "V12",
    label: "Secret scan",
    cmd: "node",
    args: ["scripts/verify-no-secrets.mjs"],
    cwd: ROOT,
  },
  {
    id: "V14",
    label: "Git whitespace check",
    cmd: "git",
    args: ["diff", "--check"],
    cwd: ROOT,
  },
  {
    id: "V15s",
    label: "Architecture gate self-test",
    cmd: "node",
    args: ["scripts/verify-v2-architecture.mjs", "--self-test"],
    cwd: ROOT,
  },
  {
    id: "V15",
    label: "Architecture constraints",
    cmd: "node",
    args: ["scripts/verify-v2-architecture.mjs"],
    cwd: ROOT,
  },
  {
    id: "V16",
    label: "Design tokens match .pen",
    cmd: "node",
    args: ["scripts/verify-design-tokens.mjs"],
    cwd: ROOT,
  },
  {
    id: "V17",
    label: "i18n contract",
    cmd: "node",
    args: ["scripts/verify-i18n.mjs"],
    cwd: ROOT,
  },
];

// V8 and V9/V10 are conditional on directories existing
if (existsSync(join(ROOT, "apps", "chimera-desktop", "package.json"))) {
  // Mirrors the CI frontend job exactly. `npm run build` is deliberately NOT
  // used: it is `tauri build`, a full signed-installer run that needs the
  // release Rust toolchain and a bundler. CI builds the web assets with
  // vite:build and compiles the shell separately in the Rust job.
  CHECKS.splice(2, 0, {
    id: "V8",
    label: "Frontend check + test + build + a11y",
    cmd: "npm",
    args: ["run", "check", "&&", "npm", "test", "&&", "npm", "run", "vite:build", "&&", "npm", "run", "test:a11y"],
    cwd: join(ROOT, "apps", "chimera-desktop"),
    shell: true,
  });
}

const results = [];
let anyFail = false;

console.log(CYAN(`\n── Chimera++ v2 Verification ──────────────────────────────`));
console.log(DIM(`Root: ${ROOT}\n`));

for (const check of CHECKS) {
  if (onlySet && !onlySet.has(check.id)) continue;
  if (skipSet.has(check.id)) {
    console.log(DIM(`  [SKIP] ${check.id} ${check.label}`));
    results.push({ id: check.id, label: check.label, status: "skip", ms: 0 });
    continue;
  }
  if (check.skipIfMissing && !existsSync(join(ROOT, check.skipIfMissing))) {
    console.log(DIM(`  [SKIP] ${check.id} ${check.label} (${check.skipIfMissing} not found)`));
    results.push({ id: check.id, label: check.label, status: "skip", ms: 0 });
    continue;
  }

  process.stdout.write(`  ${DIM("...")} ${check.id} ${check.label}`);
  const t0 = Date.now();
  const r = spawnSync(check.cmd, check.args, {
    cwd: check.cwd,
    stdio: "pipe",
    shell: !!check.shell,
    encoding: "utf8",
  });
  const ms = Date.now() - t0;
  const ok = r.status === 0;
  if (!ok) anyFail = true;

  process.stdout.write(`\r  ${ok ? GREEN("✓") : RED("✗")} ${check.id} ${check.label} ${DIM(`(${ms}ms)`)}\n`);

  if (!ok) {
    const out = (r.stdout || "") + (r.stderr || "");
    const lines = out.trim().split("\n").slice(0, 20);
    for (const line of lines) console.log(`     ${DIM("|")} ${line}`);
    if (out.trim().split("\n").length > 20) console.log(`     ${DIM("| ... (truncated)")}`);
  }

  results.push({ id: check.id, label: check.label, status: ok ? "pass" : "fail", ms });
}

// Summary table
console.log(CYAN(`\n── Summary ────────────────────────────────────────────────`));
const colW = Math.max(...results.map(r => r.id.length)) + 2;
for (const r of results) {
  const icon = r.status === "pass" ? GREEN("PASS") : r.status === "skip" ? DIM("SKIP") : RED("FAIL");
  console.log(`  ${r.id.padEnd(colW)} ${icon}  ${DIM(r.status === "skip" ? "" : `${r.ms}ms`)}  ${r.label}`);
}

const passed  = results.filter(r => r.status === "pass").length;
const failed  = results.filter(r => r.status === "fail").length;
const skipped = results.filter(r => r.status === "skip").length;
console.log(`\n  Total: ${passed} passed, ${failed} failed, ${skipped} skipped`);
console.log(anyFail ? RED("\n  VERIFICATION FAILED") : GREEN("\n  VERIFICATION PASSED"));

process.exit(anyFail ? 1 : 0);
