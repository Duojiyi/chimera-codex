// Design-verification harness — Chimera++ 2.0
//
// Loads the built frontend in a real browser at the design viewport, captures a
// screenshot of each screen next to its Pencil reference export, and asserts
// the geometry the design file pins exactly.
//
// This is a LOCAL tool, deliberately not part of CI: it needs a browser binary
// and a built `dist/`. CI enforces the static half of the same contract via
// scripts/verify-design-tokens.mjs (V16), which needs neither.
//
// Usage:
//   cd scripts/design-verify && npm install --ignore-scripts
//   node verify.mjs            # assert + capture
//   node verify.mjs --open     # also leave the browser open on Home
//
// Reference exports live in docs/design/reference/<frameId>.png and come from
// the .pen file via mcp_pencil_export_nodes — never hand-drawn.

import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { mkdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..", "..");
const APP = join(ROOT, "apps", "chimera-desktop");
const OUT = join(ROOT, "docs", "design", "actual");

// The design frames are authored at exactly this size.
const VIEWPORT = { width: 1280, height: 800 };

// Distinct from a11y.mjs's port so both harnesses can run without colliding.
const PORT = 4173;
const BASE = `http://127.0.0.1:${PORT}`;

// Screen id → Pencil frame id, so each capture sits beside its reference.
const SCREENS = [
  { id: "home", frame: "qUByL" },
  { id: "providers", frame: "yHZ03" },
  { id: "codex", frame: "EiZEM" },
  { id: "appearance", frame: "JFqAh" },
  { id: "settings", frame: "upLkQ" },
];

// Ground truth from the .pen file. Same numbers V16 asserts statically; here we
// check the browser actually renders them.
const PEN = {
  railHeight: 48,
  tabCount: 5,
  pageBackground: "rgb(12, 12, 12)", // #0C0C0C
  accent: "rgb(255, 77, 61)", // #FF4D3D
  fontFamily: /Outfit/,
  heroHeight: 360, // Home only
};

let failures = 0;
const ok = (m) => console.log(`\x1b[32m✓\x1b[0m ${m}`);
const bad = (m) => {
  failures++;
  console.log(`\x1b[31m✗\x1b[0m ${m}`);
};

function eq(label, actual, expected) {
  if (actual === expected) ok(`${label} = ${expected}`);
  else bad(`${label}: got ${JSON.stringify(actual)}, .pen says ${JSON.stringify(expected)}`);
}

function matches(label, actual, re) {
  if (re.test(String(actual))) ok(`${label} matches ${re}`);
  else bad(`${label}: got ${JSON.stringify(actual)}, expected to match ${re}`);
}

// ── Serve the built frontend ─────────────────────────────────────────────────
// `vite preview` serves the real production bundle, so what we measure is what
// ships — not a dev-server-only rendering.
// Spawn vite's own binary rather than going through `npm run`. With npm the
// process handle we hold is the npm wrapper, so killing it orphans the real
// server and the next run dies on "port already in use".
function startPreview() {
  const viteBin = join(APP, "node_modules", "vite", "bin", "vite.js");
  const proc = spawn(
    process.execPath,
    [viteBin, "preview", "--host", "127.0.0.1", "--port", String(PORT), "--strictPort"],
    { cwd: APP, stdio: ["ignore", "pipe", "pipe"] },
  );

  return new Promise((resolve, reject) => {
    let output = "";
    const timer = setTimeout(
      () => reject(new Error(`preview server did not start in 30s. Output:\n${output}`)),
      30_000,
    );
    const onData = (buf) => {
      output += buf.toString();
      // vite colourises its banner, and the ANSI bold sequence lands *between*
      // the colon and the port digits (`:\x1b[1m4173`). Strip escapes before
      // matching or the port is never found as a contiguous string.
      const plain = output.replace(/\x1b\[[0-9;]*m/g, "");
      if (plain.includes(`:${PORT}`) && /Local:/i.test(plain)) {
        clearTimeout(timer);
        resolve(proc);
      }
    };
    proc.stdout.on("data", onData);
    proc.stderr.on("data", onData);
    proc.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`preview server exited with code ${code}. Output:\n${output}`));
    });
  });
}

// ── Run ──────────────────────────────────────────────────────────────────────
if (!existsSync(join(APP, "dist", "index.html"))) {
  console.error("dist/ is missing — run `npm run vite:build` in apps/chimera-desktop first.");
  process.exit(1);
}
mkdirSync(OUT, { recursive: true });

let server;
let browser;
try {
  server = await startPreview();
  browser = await chromium.launch();
  const page = await browser.newPage({ viewport: VIEWPORT, deviceScaleFactor: 2 });

  for (const { id, frame } of SCREENS) {
    console.log(`\n── ${id} ──`);
    await page.goto(`${BASE}/?screen=${id}&lang=zh`, { waitUntil: "load" });
    // Webfonts must be resolved before measuring or screenshotting, or the
    // first paint falls back and every type metric is wrong.
    await page.evaluate(() => document.fonts.ready);

    // Shell geometry — identical on all five screens per the .pen file.
    const rail = await page.locator("nav[role=navigation]").first();
    eq(`${id}: rail height`, Math.round((await rail.boundingBox()).height), PEN.railHeight);
    eq(
      `${id}: nav tab count`,
      // Scope to the rail. Features build their own vertical tablists (provider
      // list, skin list, settings categories), so a document-wide count is
      // meaningless here.
      await page.locator("nav[role=navigation] [role=tab]").count(),
      PEN.tabCount,
    );

    const bodyBg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    eq(`${id}: page background`, bodyBg, PEN.pageBackground);

    const bodyFont = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
    matches(`${id}: body font`, bodyFont, PEN.fontFamily);

    // The active tab must carry the accent underline — the single accent rule.
    const activeBorder = await page.evaluate(() => {
      const el = document.querySelector('nav[role=navigation] [role=tab][aria-selected="true"]');
      return el ? getComputedStyle(el).borderBottomColor : null;
    });
    eq(`${id}: active tab underline`, activeBorder, PEN.accent);

    // Screen-specific anchor.
    if (id === "home") {
      const heroH = await page.evaluate(() => {
        const main = document.querySelector("main");
        const hero = main?.firstElementChild?.firstElementChild;
        return hero ? Math.round(hero.getBoundingClientRect().height) : null;
      });
      eq("home: hero height", heroH, PEN.heroHeight);
    }

    await page.screenshot({ path: join(OUT, `${id}.png`) });
    ok(`${id}: captured → docs/design/actual/${id}.png (ref: ${frame}.png)`);
  }

  // Both languages must render without layout collapse. Chinese is the default,
  // so English is the case that can overflow a fixed-width label.
  console.log("\n── i18n render check ──");
  for (const lang of ["zh", "en"]) {
    await page.goto(`${BASE}/?screen=settings&lang=${lang}`, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const overflow = await page.evaluate(() => document.body.scrollWidth > window.innerWidth);
    if (overflow) bad(`settings/${lang}: content overflows the 1280px viewport`);
    else ok(`settings/${lang}: no horizontal overflow`);
    await page.screenshot({ path: join(OUT, `settings-${lang}.png`) });
  }

  if (process.argv.includes("--open")) {
    console.log("\nLeaving the browser open on Home. Ctrl-C to exit.");
    await page.goto(`${BASE}/?screen=home&lang=zh`);
    await new Promise(() => {});
  }
} finally {
  await browser?.close();
  server?.kill();
}

console.log(
  failures === 0
    ? "\n\x1b[32m✓ design-verify: PASS\x1b[0m"
    : `\n\x1b[31m✗ design-verify: FAIL (${failures})\x1b[0m`,
);
process.exit(failures === 0 ? 0 : 1);
