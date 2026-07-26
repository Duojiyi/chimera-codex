#!/usr/bin/env node
// V16 — Design token conformance gate (Chimera++ 2.0)
//
// Why this exists: during Task 1/3 the React features were written from memory
// of the Pencil design file instead of from the file itself. The result drifted
// (88px hero rendered as 80px, danger colour indistinguishable from the accent,
// the Outfit font declared in CSS but never installed). Tests did not catch it
// because none of it is behavioural.
//
// This gate makes the drift mechanical rather than a matter of discipline:
//   1. Only src/design/tokens.ts may contain raw hex colour literals.
//   2. The token values must equal the values extracted from the .pen file.
//   3. The font declared in CSS must actually be an installed dependency.
//
// Cross-platform (ADR-007): Node only, identical on Windows and macOS runners.

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, sep } from "node:path";

const ROOT = process.cwd();
const APP = join(ROOT, "apps", "chimera-desktop");
const SRC = join(APP, "src");
const TOKENS_FILE = join(SRC, "design", "tokens.ts");

const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const RESET = "\x1b[0m";

let failures = 0;
const pass = (m) => console.log(`${GREEN}✓${RESET} ${m}`);
const fail = (m) => {
  console.log(`${RED}✗${RESET} ${m}`);
  failures += 1;
};

// ── Ground truth: values read out of the .pen design file ────────────────────
// Source: chimera-v2-screens.pen, frames Home/Providers/Codex/Appearance/Settings.
// Any change here must come from the design file, never from a guess.
const PEN_COLORS = {
  ink0: "#0C0C0C",
  ink1: "#111111",
  ink2: "#181818",
  ink3: "#222222",
  rule: "#282828",
  primary: "#EBEBEB",
  accent: "#FF4D3D",
  green: "#34C759",
  amber: "#FF9F0A",
  danger: "#FF453A",
};

// Deliberate WCAG 2.2 AA overrides of the .pen text greys. The design file's
// values fail the 4.5:1 minimum for normal-size text, and these tones carry
// real structure (section labels, subtitles, version string), so they cannot be
// hidden from assistive tech instead. Each entry records the .pen original so
// the divergence stays visible, and the contrast check below proves the
// replacement actually earns its place — a wrong value fails the gate.
const A11Y_OVERRIDES = {
  secondary: { pen: "#999999", use: "#B8B8B8" },
  muted: { pen: "#5E5E5E", use: "#9A9A9A" },
  dim: { pen: "#3A3A3A", use: "#8A8A8A" },
};

/**
 * Every surface text sits on, from the .pen file. The override must clear AA
 * against all of them — checking only the page background would let a token
 * pass while failing on a panel or a selected row.
 */
const SURFACES = {
  ink0: "#0C0C0C",
  ink1: "#111111",
  ink2: "#181818",
  ink3: "#222222",
};
const AA_NORMAL = 4.5;

function relativeLuminance(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrastRatio(a, b) {
  const [x, y] = [relativeLuminance(a), relativeLuminance(b)];
  const [hi, lo] = x > y ? [x, y] : [y, x];
  return (hi + 0.05) / (lo + 0.05);
}

// Display-size anchors: each screen's single largest type element.
const PEN_ANCHORS = {
  hero: 88,          // Home — provider name
  version: 72,       // Codex — managed runtime version
  detailTitle: 44,   // Providers — detail title
  pageTitle: 36,     // Settings — page title
  skinTitle: 30,     // Appearance — skin detail title
};

const PEN_RAIL_HEIGHT = 48;
const PEN_EYEBROW_TRACKING = 1.5;

// ── 1. tokens.ts must exist and match the .pen values ───────────────────────
if (!existsSync(TOKENS_FILE)) {
  fail("apps/chimera-desktop/src/design/tokens.ts is missing");
} else {
  const tokens = readFileSync(TOKENS_FILE, "utf8");

  for (const [name, hex] of Object.entries(PEN_COLORS)) {
    // Match  "ink-0": "#0C0C0C"  or  rule: "#282828"
    const key = name.includes("-") ? `"${name}"` : name;
    const re = new RegExp(`${key}\\s*:\\s*"(#[0-9A-Fa-f]{6,8})"`);
    const found = tokens.match(re);
    if (!found) {
      fail(`tokens.ts: colour token \`${name}\` not found`);
    } else if (found[1].toUpperCase() !== hex.toUpperCase()) {
      fail(`tokens.ts: \`${name}\` is ${found[1]}, .pen file says ${hex}`);
    }
  }
  if (failures === 0) pass(`all ${Object.keys(PEN_COLORS).length} colour tokens match the .pen file`);

  // Overrides must (a) be present at the stated value and (b) actually earn the
  // exemption by clearing 4.5:1 on every surface. An override that does not
  // improve contrast is just drift wearing a comment.
  for (const [name, o] of Object.entries(A11Y_OVERRIDES)) {
    const re = new RegExp(`${name}\\s*:\\s*"(#[0-9A-Fa-f]{6})"`);
    const found = tokens.match(re);
    if (!found) {
      fail(`tokens.ts: override token \`${name}\` not found`);
      continue;
    }
    if (found[1].toUpperCase() !== o.use.toUpperCase()) {
      fail(`tokens.ts: \`${name}\` is ${found[1]}, the recorded a11y override is ${o.use}`);
      continue;
    }
    let worst = Infinity;
    let worstBg = "";
    for (const [bgName, bg] of Object.entries(SURFACES)) {
      const cr = contrastRatio(found[1], bg);
      if (cr < worst) {
        worst = cr;
        worstBg = bgName;
      }
    }
    if (worst < AA_NORMAL) {
      fail(
        `tokens.ts: \`${name}\` (${o.use}) is ${worst.toFixed(2)}:1 on ${worstBg} — ` +
          `below AA ${AA_NORMAL}:1, so the override does not achieve its stated purpose`,
      );
    } else {
      pass(
        `override ${name} ${o.pen}→${o.use}: ${worst.toFixed(2)}:1 worst-case (${worstBg}), clears AA`,
      );
    }
  }

  for (const [name, size] of Object.entries(PEN_ANCHORS)) {
    // Anchors are nested:  hero: { fontSize: 88, fontWeight: 700, ... }
    const re = new RegExp(`${name}\\s*:\\s*\\{\\s*fontSize:\\s*(\\d+)`);
    const found = tokens.match(re);
    if (!found) {
      fail(`tokens.ts: type anchor \`${name}\` not found`);
    } else if (Number(found[1]) !== size) {
      fail(`tokens.ts: anchor \`${name}\` is ${found[1]}px, .pen file says ${size}px`);
    } else {
      pass(`type anchor ${name} = ${size}px matches .pen`);
    }
  }

  // size.rail — top rail height
  if (!new RegExp(`rail\\s*:\\s*${PEN_RAIL_HEIGHT}\\b`).test(tokens)) {
    fail(`tokens.ts: size.rail must be ${PEN_RAIL_HEIGHT} per .pen file`);
  } else {
    pass(`rail height = ${PEN_RAIL_HEIGHT}px matches .pen`);
  }

  // Both eyebrow and sectionLabel carry letterSpacing 1.5 in the .pen file.
  const trackingCount = (tokens.match(
    new RegExp(`letterSpacing\\s*:\\s*${PEN_EYEBROW_TRACKING}\\b`, "g"),
  ) ?? []).length;
  if (trackingCount < 2) {
    fail(
      `tokens.ts: expected eyebrow AND sectionLabel to declare letterSpacing ${PEN_EYEBROW_TRACKING} per .pen file (found ${trackingCount})`,
    );
  } else {
    pass(`uppercase tracking = ${PEN_EYEBROW_TRACKING} on ${trackingCount} label styles`);
  }
}

// ── 2. No bare hex literals outside tokens.ts ───────────────────────────────
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

const HEX_RE = /#[0-9A-Fa-f]{3,8}\b/g;
const offenders = [];

/**
 * Remove comments so the gate only inspects code that actually renders.
 * Documenting a colour value in a comment (e.g. explaining why a token was
 * chosen over a mock's value) is legitimate and must not fail the build.
 * Handles `//` line comments, block comments, and JSX `{/* ... *\/}` blocks,
 * while leaving `//` inside string literals alone so URLs survive.
 */
function stripComments(src) {
  let out = "";
  let i = 0;
  const n = src.length;
  let quote = null; // active string delimiter: ' " or `

  while (i < n) {
    const c = src[i];
    const next = src[i + 1];

    if (quote) {
      if (c === "\\") {
        out += c + (next ?? "");
        i += 2;
        continue;
      }
      if (c === quote) quote = null;
      out += c;
      i += 1;
      continue;
    }

    if (c === '"' || c === "'" || c === "`") {
      quote = c;
      out += c;
      i += 1;
      continue;
    }

    if (c === "/" && next === "/") {
      while (i < n && src[i] !== "\n") i += 1;
      continue;
    }

    if (c === "/" && next === "*") {
      i += 2;
      while (i < n && !(src[i] === "*" && src[i + 1] === "/")) i += 1;
      i += 2;
      continue;
    }

    out += c;
    i += 1;
  }
  return out;
}

for (const file of walk(SRC)) {
  const rel = relative(ROOT, file).split(sep).join("/");
  if (rel.endsWith("src/design/tokens.ts")) continue; // the one allowed home
  if (/\.test\.tsx?$/.test(rel)) continue; // test fixtures may assert on values

  const text = stripComments(readFileSync(file, "utf8"));
  const hits = text.match(HEX_RE);
  if (hits) {
    offenders.push({ rel, hits: [...new Set(hits)] });
  }
}

if (offenders.length > 0) {
  for (const o of offenders) {
    fail(`${o.rel}: raw hex literal(s) ${o.hits.join(", ")} — import from design/tokens.ts instead`);
  }
} else {
  pass("no raw hex colour literals outside design/tokens.ts");
}

// ── 3. The declared font must actually be installed ─────────────────────────
const cssPath = join(SRC, "styles.css");
const pkgPath = join(APP, "package.json");

if (!existsSync(cssPath)) {
  fail("apps/chimera-desktop/src/styles.css is missing");
} else if (!existsSync(pkgPath)) {
  fail("apps/chimera-desktop/package.json is missing");
} else {
  const css = readFileSync(cssPath, "utf8");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  const deps = { ...(pkg.dependencies ?? {}), ...(pkg.devDependencies ?? {}) };

  // Pull the first family out of the body font-family declaration.
  const famMatch = css.match(/font-family:\s*["']([^"']+)["']/);
  if (!famMatch) {
    fail("styles.css: no quoted font-family declaration found");
  } else {
    const family = famMatch[1];
    const slug = family.toLowerCase().replace(/\s+/g, "-");
    const fontsourcePkg = `@fontsource/${slug}`;
    const declaredInCss = new RegExp(`@fontsource/${slug}`).test(css);
    const installed = Object.hasOwn(deps, fontsourcePkg);

    if (!installed) {
      fail(
        `styles.css declares font "${family}" but ${fontsourcePkg} is not a dependency — ` +
          `the app would silently fall back to a system font`,
      );
    } else if (!declaredInCss) {
      fail(`${fontsourcePkg} is installed but styles.css never @imports its weights`);
    } else {
      pass(`font "${family}" is declared, imported, and installed (${fontsourcePkg})`);
    }
  }
}

// ── Result ──────────────────────────────────────────────────────────────────
console.log("");
if (failures > 0) {
  console.log(`${RED}✗ verify-design-tokens: FAIL (${failures} finding${failures === 1 ? "" : "s"})${RESET}`);
  process.exit(1);
}
console.log(`${GREEN}✓ verify-design-tokens: PASS${RESET}`);
