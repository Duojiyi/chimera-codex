#!/usr/bin/env node
// V17 — i18n contract gate.
//
// Enforces four properties:
//   1. zh.ts and en.ts cover exactly the same key set (no orphans either way).
//   2. No translation value is empty.
//   3. No user-facing literal is hardcoded in a feature/shell component —
//      every one must come through t()/tf().
//   4. t() is not called at module scope, which would freeze the string at
//      import time and break instant language switching.
//
// Property 4 is what lets v2 switch language without reloading the webview,
// unlike the 1.x manager. Cross-platform Node only, per ADR-007.

import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const APP = join(ROOT, "apps", "chimera-desktop");
const SRC = join(APP, "src");
const I18N = join(SRC, "i18n");

const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const RESET = "\x1b[0m";

let failures = 0;
const fail = (m) => {
  console.log(`${RED}✗${RESET} ${m}`);
  failures++;
};
const pass = (m) => console.log(`${GREEN}✓${RESET} ${m}`);

// ── 1 + 2. Dictionary parity and non-empty values ───────────────────────────
// Parsed with a regex rather than imported: this script must run without a
// TypeScript loader on both Windows and macOS runners.
function parseKeys(file) {
  if (!existsSync(file)) return null;
  const text = readFileSync(file, "utf8");
  const body = text.slice(text.indexOf("{"), text.lastIndexOf("}") + 1);
  const entries = new Map();
  // Matches:  "key.name": "value",   with the value allowed to contain escapes.
  const re = /"([A-Za-z0-9_.]+)"\s*:\s*"((?:[^"\\]|\\.)*)"/g;
  let m;
  while ((m = re.exec(body)) !== null) {
    entries.set(m[1], m[2]);
  }
  return entries;
}

const zhFile = join(I18N, "zh.ts");
const enFile = join(I18N, "en.ts");
const zhEntries = parseKeys(zhFile);
const enEntries = parseKeys(enFile);

if (!zhEntries || !enEntries) {
  fail("i18n/zh.ts and i18n/en.ts must both exist");
} else {
  const zhKeys = new Set(zhEntries.keys());
  const enKeys = new Set(enEntries.keys());

  const missingInEn = [...zhKeys].filter((k) => !enKeys.has(k));
  const missingInZh = [...enKeys].filter((k) => !zhKeys.has(k));

  if (missingInEn.length > 0) {
    fail(`en.ts is missing ${missingInEn.length} key(s): ${missingInEn.slice(0, 8).join(", ")}`);
  }
  if (missingInZh.length > 0) {
    fail(`zh.ts is missing ${missingInZh.length} key(s): ${missingInZh.slice(0, 8).join(", ")}`);
  }
  if (missingInEn.length === 0 && missingInZh.length === 0) {
    pass(`zh.ts and en.ts both define the same ${zhKeys.size} keys`);
  }

  for (const [dictName, entries] of [["zh", zhEntries], ["en", enEntries]]) {
    const empty = [...entries.entries()].filter(([, v]) => v.trim() === "").map(([k]) => k);
    if (empty.length > 0) {
      fail(`${dictName}.ts has empty value(s): ${empty.slice(0, 8).join(", ")}`);
    }
  }

  // Placeholder arity must agree, or interpolation silently drops an argument.
  const arity = (v) => new Set([...v.matchAll(/\{(\d+)\}/g)].map((m) => m[1])).size;
  const mismatched = [...zhEntries.entries()]
    .filter(([k, v]) => enEntries.has(k) && arity(v) !== arity(enEntries.get(k)))
    .map(([k]) => k);
  if (mismatched.length > 0) {
    fail(`placeholder count differs between zh/en for: ${mismatched.join(", ")}`);
  } else {
    pass("placeholder arity matches between zh and en");
  }
}

// ── 3 + 4. Component-level rules ────────────────────────────────────────────
function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "node_modules" || entry === "dist") continue;
      walk(full, out);
    } else if (/\.tsx?$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

/** Strip comments and import lines so they are never scanned as code. */
function strip(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, "")
    .replace(/^\s*import\s[\s\S]*?from\s+["'][^"']+["'];?\s*$/gm, "");
}

const componentDirs = [join(SRC, "features"), join(SRC, "shell")];
const files = componentDirs.flatMap((d) => walk(d));

// Rule 4: t(...) must not appear at module scope (column 0 indentation inside
// a top-level const/array/object). Detected by finding t( before the first
// exported function in the file.
const moduleScopeOffenders = [];
// Rule 3: JSX text and user-facing attributes must not be literal strings.
const literalOffenders = [];

// Attributes a screen reader or the user reads directly.
const USER_FACING_ATTRS = /(?:aria-label|placeholder|title|aria-description)\s*=\s*"([^"]{2,})"/g;
// JSX text between tags: >Some text<
const JSX_TEXT = />([^<>{}\n]*[A-Za-z]{2}[^<>{}\n]*)</g;

for (const file of files) {
  const rel = relative(ROOT, file).split(sep).join("/");
  if (/\.test\.tsx?$/.test(rel)) continue;
  if (rel.includes("/lib/")) continue; // pure logic, no user-facing output

  const raw = readFileSync(file, "utf8");
  // Strip TypeScript generic argument lists before the JSX scan. In
  // `Record<string, unknown>) => Promise<unknown>` the span between the two
  // `>`/`<` reads as JSX text to the regex below, producing a phantom
  // `text "Promise"`. A generic is `Ident<...>` with no `/` inside; a JSX
  // closing tag always has one, so this cannot eat real markup.
  const text = strip(raw).replace(/\b[A-Z][A-Za-z0-9_]*<[^<>/]*>/g, "GENERIC");

  // Rule 4 — find module scope: everything before the first `export function`.
  const firstExport = text.search(/^export\s+(?:default\s+)?function/m);
  const moduleScope = firstExport === -1 ? text : text.slice(0, firstExport);
  if (/\bt\(|\btf\(|\btranslateStatic\(/.test(moduleScope)) {
    moduleScopeOffenders.push(rel);
  }

  const hits = new Set();
  for (const m of text.matchAll(USER_FACING_ATTRS)) {
    // Values that are pure punctuation/symbols are not translatable text.
    if (/[A-Za-z]{2}/.test(m[1])) hits.add(`${m[0].split("=")[0]}="${m[1]}"`);
  }
  for (const m of text.matchAll(JSX_TEXT)) {
    const s = m[1].trim();
    if (s.length < 2) continue;
    // Skip pure symbols/arrows and single-word CSS-ish tokens.
    if (!/[A-Za-z]{2}/.test(s)) continue;
    hits.add(`text "${s}"`);
  }
  if (hits.size > 0) {
    literalOffenders.push({ rel, hits: [...hits] });
  }
}

if (moduleScopeOffenders.length > 0) {
  for (const rel of moduleScopeOffenders) {
    fail(
      `${rel}: t()/tf() called at module scope — freezes the string at import ` +
        `and breaks instant switching. Store the key, translate inside the component.`,
    );
  }
} else {
  pass("no translator called at module scope (instant switching stays safe)");
}

if (literalOffenders.length > 0) {
  // Print every hit. An earlier revision capped this at 4 per file, and four
  // copies of one false positive filled the slice in every file — which hid
  // that home/index.tsx had never been translated at all. Truncation that
  // silently drops findings reads as "clean" when it is not.
  for (const o of literalOffenders) {
    fail(`${o.rel}: ${o.hits.length} untranslated literal(s)`);
    for (const h of o.hits) console.log(`     | ${h}`);
  }
} else {
  pass(`no hardcoded user-facing literals in ${files.length} component file(s)`);
}

// ── Result ──────────────────────────────────────────────────────────────────
console.log("");
if (failures > 0) {
  console.log(`${RED}✗ verify-i18n: FAIL (${failures} finding${failures === 1 ? "" : "s"})${RESET}`);
  process.exit(1);
}
console.log(`${GREEN}✓ verify-i18n: PASS${RESET}`);
