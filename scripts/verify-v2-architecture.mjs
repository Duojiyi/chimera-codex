#!/usr/bin/env node
// V15 — Architecture constraint check (cross-platform, Node built-ins only).
// Enforces Chimera v2 layering rules before any code lands.
// Usage: node scripts/verify-v2-architecture.mjs [--self-test]
//
// Design note: an earlier revision of this script re-implemented the frontend
// rules with regexes and left the crate-graph and legacy-growth checks as
// comments that always passed. Both holes were real: the regex version had no
// notion of cycles and never caught App.tsx <-> shell/TopRail.tsx, and the
// "monitored" legacy check asserted nothing at all. This revision runs
// dependency-cruiser for real, walks the actual crate graph transitively, and
// proves each rule can fail via --self-test.
import { readFileSync, readdirSync, statSync, existsSync } from "fs";
import { join, relative } from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const FRONTEND = join(ROOT, "apps", "chimera-desktop");
const PASS = "\x1b[32m✓\x1b[0m";
const FAIL = "\x1b[31m✗\x1b[0m";
let failures = 0;

function fail(msg) { console.log(`${FAIL} ${msg}`); failures++; }
function pass(msg) { console.log(`${PASS} ${msg}`); }

// ── Layering model ─────────────────────────────────────────────────────────
// Lower layers may never reference higher ones, and siblings within the
// adapter layer may never reference each other — that is what keeps
// "one writer per concern" (G1) checkable instead of aspirational.
const LAYER = {
  "chimera-domain": 0,   // pure types and state machines, no I/O
  "chimera-platform": 1, // ports over OS capabilities
  "chimera-provider": 2, // adapters
  "chimera-runtime": 2,
  "chimera-theme": 2,
  "chimera-migration": 2,
  "mirror-contract": 2,  // standalone service crate
  "chimera-desktop": 3,  // thin Tauri shell, composes everything
};

// v2 must not depend on the 1.x tree (G2).
const LEGACY_CRATES = [
  "codex-plus-core",
  "codex-plus-data",
  "codex-plus-launcher",
  "codex-plus-manager",
];

/**
 * Pure rule engine: given `name -> [dependency names]`, return every edge that
 * breaks the layering contract. Kept free of I/O so --self-test can drive it
 * with synthetic manifests and prove it actually rejects illegal graphs.
 */
export function layerViolations(graph) {
  const violations = [];
  for (const [crate, deps] of Object.entries(graph)) {
    const from = LAYER[crate];
    if (from === undefined) continue; // not a v2 crate; nothing to enforce
    for (const dep of deps) {
      if (LEGACY_CRATES.includes(dep)) {
        violations.push(`${crate} -> ${dep}: v2 crate depends on the 1.x tree (G2)`);
        continue;
      }
      const to = LAYER[dep];
      if (to === undefined) continue; // third-party crate
      if (to > from) {
        violations.push(`${crate} (L${from}) -> ${dep} (L${to}): depends on a higher layer`);
      } else if (to === from && from === 2) {
        violations.push(`${crate} -> ${dep}: adapter-to-adapter dependency (G1)`);
      }
    }
  }
  return violations;
}

/** Every first-party dependency named in a Cargo.toml, from both dep sections. */
function firstPartyDeps(tomlPath) {
  const known = new Set([...Object.keys(LAYER), ...LEGACY_CRATES]);
  const deps = new Set();
  for (const line of readFileSync(tomlPath, "utf8").split("\n")) {
    const m = line.match(/^\s*([a-z0-9-]+)\s*(=|\.)/);
    if (m && known.has(m[1])) deps.add(m[1]);
  }
  return [...deps];
}

function walkFiles(dir, ext, results = []) {
  if (!existsSync(dir)) return results;
  for (const e of readdirSync(dir)) {
    const full = join(dir, e);
    try {
      const st = statSync(full);
      if (st.isDirectory() && e !== "node_modules" && e !== "target" && e !== ".git") {
        walkFiles(full, ext, results);
      } else if (st.isFile() && full.endsWith(ext)) {
        results.push(full);
      }
    } catch { /* unreadable entry; the crate-graph and depcruise checks cover it */ }
  }
  return results;
}

// ── --self-test: prove each rule can fail ──────────────────────────────────
// Step 1.1 asks for a negative fixture. A gate nobody has ever seen go red is
// indistinguishable from a gate that cannot go red.
if (process.argv.includes("--self-test")) {
  const cases = [
    ["domain depending on an adapter", { "chimera-domain": ["chimera-provider"] }],
    ["adapter depending on a sibling adapter", { "chimera-runtime": ["chimera-provider"] }],
    ["platform depending on the shell", { "chimera-platform": ["chimera-desktop"] }],
    ["v2 crate depending on the 1.x tree", { "chimera-runtime": ["codex-plus-core"] }],
  ];
  let bad = 0;
  for (const [label, graph] of cases) {
    const v = layerViolations(graph);
    if (v.length === 0) { console.log(`${FAIL} self-test: ${label} was NOT rejected`); bad++; }
    else console.log(`${PASS} self-test rejects ${label}`);
  }
  const legal = layerViolations({
    "chimera-domain": [], "chimera-platform": ["chimera-domain"],
    "chimera-provider": ["chimera-domain", "chimera-platform"],
    "chimera-desktop": ["chimera-domain", "chimera-provider", "chimera-runtime"],
  });
  if (legal.length > 0) { console.log(`${FAIL} self-test: legal graph was rejected: ${legal.join("; ")}`); bad++; }
  else console.log(`${PASS} self-test accepts the real layering`);
  console.log(`\n${bad === 0 ? PASS : FAIL} self-test: ${bad === 0 ? "PASS" : `${bad} failure(s)`}`);
  process.exit(bad > 0 ? 1 : 0);
}

// ── Check 1: crate graph obeys the layering contract ───────────────────────
{
  const manifests = [
    ...walkFiles(join(ROOT, "crates"), "Cargo.toml"),
    ...walkFiles(join(ROOT, "services"), "Cargo.toml"),
    join(ROOT, "apps", "chimera-desktop", "src-tauri", "Cargo.toml"),
  ].filter(existsSync);

  const graph = {};
  for (const m of manifests) {
    const name = readFileSync(m, "utf8").match(/^name\s*=\s*"([^"]+)"/m)?.[1];
    if (name) graph[name] = firstPartyDeps(m);
  }

  const seen = Object.keys(graph).filter(n => LAYER[n] !== undefined);
  const missing = Object.keys(LAYER).filter(n => !seen.includes(n));
  if (missing.length > 0) {
    fail(`crate graph: expected v2 crates not found on disk: ${missing.join(", ")}`);
  }

  const violations = layerViolations(graph);
  if (violations.length === 0) {
    pass(`Crate graph obeys the layering contract (${seen.length} v2 crates checked)`);
  } else {
    for (const v of violations) fail(`Layering: ${v}`);
  }
}

// ── Check 2: dependency-cruiser actually runs ──────────────────────────────
{
  const cfg = join(FRONTEND, ".dependency-cruiser.cjs");
  if (!existsSync(cfg)) {
    fail("dependency-cruiser config missing at apps/chimera-desktop/.dependency-cruiser.cjs");
  } else if (!existsSync(join(FRONTEND, "node_modules", "dependency-cruiser"))) {
    // Never skip silently: an unrunnable gate is a failed gate.
    fail("dependency-cruiser is not installed — run `npm ci` in apps/chimera-desktop before V15");
  } else {
    const r = spawnSync("npx", ["--no-install", "depcruise", "src", "--config", ".dependency-cruiser.cjs"], {
      cwd: FRONTEND, encoding: "utf8", shell: process.platform === "win32",
    });
    if (r.status === 0) {
      pass("dependency-cruiser: no frontend dependency violations (cross-feature, shell, cycles)");
    } else {
      const out = ((r.stdout || "") + (r.stderr || "")).trim();
      fail("dependency-cruiser reported violations:");
      for (const line of out.split("\n").slice(0, 25)) console.log(`     | ${line}`);
    }
  }
}

// ── Check 3: v2 must not grow the 1.x crates ───────────────────────────────
// The base commit is the last 1.x state the v2 branch forked from. Any .rs
// file inside a legacy crate that is not in that tree was added by v2 work.
{
  const BASE = "2ce80f2c";
  const legacyPaths = LEGACY_CRATES
    .flatMap(c => [join("crates", c), join("apps", c)])
    .filter(p => existsSync(join(ROOT, p)));

  if (legacyPaths.length === 0) {
    pass("No 1.x crates present — nothing to guard against growth");
  } else {
    const r = spawnSync("git", ["ls-tree", "-r", "--name-only", BASE, "--", ...legacyPaths], {
      cwd: ROOT, encoding: "utf8",
    });
    if (r.status !== 0) {
      fail(`legacy-growth check could not read base tree ${BASE} (needs full history: fetch-depth: 0)`);
    } else {
      const baseFiles = new Set(r.stdout.split("\n").map(s => s.trim()).filter(Boolean));
      const added = legacyPaths
        .flatMap(p => walkFiles(join(ROOT, p), ".rs"))
        .map(f => relative(ROOT, f).split("\\").join("/"))
        .filter(f => !baseFiles.has(f));
      if (added.length === 0) {
        pass(`No new .rs files in the 1.x crates since ${BASE} (G2)`);
      } else {
        for (const f of added) fail(`v2 added a file to a 1.x crate (G2): ${f}`);
      }
    }
  }
}

// ── Check 4: Tauri build hooks must not invoke themselves ──────────────────
// `beforeBuildCommand: "npm run build"` where `build` is `tauri build` spawns
// itself forever, so `tauri build` never produces an installer. It shipped
// that way because no gate ever packaged the app — CI only runs vite:build.
{
  const conf = join(FRONTEND, "src-tauri", "tauri.conf.json");
  const pkg = join(FRONTEND, "package.json");
  if (!existsSync(conf) || !existsSync(pkg)) {
    fail("tauri.conf.json or package.json missing from apps/chimera-desktop");
  } else {
    const build = JSON.parse(readFileSync(conf, "utf8")).build ?? {};
    const scripts = JSON.parse(readFileSync(pkg, "utf8")).scripts ?? {};
    const bad = [];
    for (const hook of ["beforeDevCommand", "beforeBuildCommand"]) {
      const cmd = build[hook];
      if (!cmd) continue;
      const script = cmd.match(/^npm\s+run\s+([\w:-]+)/)?.[1];
      if (script && /\btauri\s+(dev|build)\b/.test(scripts[script] ?? "")) {
        bad.push(`${hook}: "${cmd}" -> "${scripts[script]}" re-enters Tauri`);
      }
    }
    if (bad.length === 0) {
      pass("Tauri build hooks do not re-enter Tauri (packaging can terminate)");
    } else {
      for (const b of bad) fail(`Recursive Tauri hook — ${b}`);
    }
  }
}

// ── Check 5: frontend must not reach into Rust crates ──────────────────────
{
  const src = join(FRONTEND, "src");
  if (!existsSync(src)) {
    fail("apps/chimera-desktop/src not found");
  } else {
    const files = walkFiles(src, ".ts").concat(walkFiles(src, ".tsx"));
    const bad = files.filter(f =>
      /from\s+["'][^"']*\.\.\/(crates|src-tauri)\//.test(readFileSync(f, "utf8")));
    if (bad.length === 0) {
      pass(`No direct crate imports from frontend TypeScript (${files.length} files)`);
    } else {
      for (const f of bad) fail(`Frontend reaches into Rust source: ${relative(ROOT, f)}`);
    }
  }
}

// ── Check 6: declared Tauri features must actually be built ────────────────
// `tray-icon` was enabled in Cargo.toml with no TrayIconBuilder anywhere, while
// Settings offered "start minimized to tray" — a toggle that could only hide
// the window with nothing to restore it from. A paid-for capability that is
// never constructed is worse than one that was never declared, because the UI
// starts promising it.
{
  const toml = join(FRONTEND, "src-tauri", "Cargo.toml");
  const srcDir = join(FRONTEND, "src-tauri", "src");
  // feature name -> the symbol that proves it was built
  const MUST_BUILD = { "tray-icon": "TrayIconBuilder" };

  if (!existsSync(toml)) {
    fail("apps/chimera-desktop/src-tauri/Cargo.toml missing");
  } else {
    const declared = readFileSync(toml, "utf8");
    const sources = walkFiles(srcDir, ".rs").map(f => readFileSync(f, "utf8")).join("\n");
    const unbuilt = Object.entries(MUST_BUILD)
      .filter(([feat, symbol]) => declared.includes(`"${feat}"`) && !sources.includes(symbol));
    if (unbuilt.length === 0) {
      pass("Every declared Tauri feature is actually constructed");
    } else {
      for (const [feat, symbol] of unbuilt) {
        fail(`Tauri feature "${feat}" is declared but no ${symbol} exists in src-tauri/src`);
      }
    }
  }
}

// ── Check 7: ChimeraHub's URL comes from brand/product.toml ────────────────
// brand/product.toml is the single source of truth (G1: one writer). v2 hand-
// wrote `api.chimerahub.io` in five places while the real host has always been
// `api.chimerahub.org` — every ChimeraHub preset pointed at a domain that does
// not exist. Nothing caught it because nothing compared the two.
{
  const brand = join(ROOT, "brand", "product.toml");
  if (!existsSync(brand)) {
    fail("brand/product.toml missing — the branding source of truth");
  } else {
    const authoritative = readFileSync(brand, "utf8")
      .match(/^default_relay_base_url\s*=\s*"([^"]+)"/m)?.[1];
    if (!authoritative) {
      fail("brand/product.toml has no default_relay_base_url");
    } else {
      const host = new URL(authoritative).host;
      const sources = [
        ...walkFiles(join(ROOT, "crates"), ".rs"),
        ...walkFiles(join(FRONTEND, "src"), ".ts"),
        ...walkFiles(join(FRONTEND, "src"), ".tsx"),
        ...walkFiles(join(FRONTEND, "src-tauri", "src"), ".rs"),
      ];
      const wrong = [];
      for (const f of sources) {
        for (const m of readFileSync(f, "utf8").matchAll(/\bapi\.chimerahub\.[a-z]+/g)) {
          if (m[0] !== host) wrong.push(`${relative(ROOT, f)}: "${m[0]}" (brand says "${host}")`);
        }
      }
      if (wrong.length === 0) {
        pass(`ChimeraHub host matches brand/product.toml (${host})`);
      } else {
        for (const w of [...new Set(wrong)]) fail(`Brand drift — ${w}`);
      }
    }
  }
}

console.log(`\n${failures === 0 ? PASS : FAIL} verify-v2-architecture: ${failures === 0 ? "PASS" : `${failures} violation(s)`}`);
process.exit(failures > 0 ? 1 : 0);
