// Design-verification harness — Chimera++ 2.0
//
// Loads the frontend in a real browser at the design viewport, captures a
// screenshot of each screen, and asserts the geometry pinned by the Soft Bento
// product design. This is intentionally a browser-level contract: token checks
// alone cannot catch a shell that renders at the wrong size.
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

const SCREENS = ["home", "providers", "codex", "appearance", "settings"];

// Ground truth from the Soft Bento desktop design. Same numbers are asserted
// statically by verify-design-tokens.mjs; here we check the browser actually
// renders them.
const PEN = {
  canvasPadding: 24,
  windowRadius: 26,
  windowBarHeight: 58,
  sidebarWidth: 232,
  tabCount: 5,
  pageBackground: "rgb(168, 207, 210)", // #A8CFD2
  windowBackground: "rgb(238, 248, 245)", // #EEF8F5
  activeSurface: "rgb(255, 255, 255)", // #FFFFFF
  fontFamily: /Outfit/,
  homeStatCount: 3,
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

  for (const id of SCREENS) {
    console.log(`\n── ${id} ──`);
    await page.goto(`${BASE}/?screen=${id}&lang=zh`, { waitUntil: "load" });
    // Webfonts must be resolved before measuring or screenshotting, or the
    // first paint falls back and every type metric is wrong.
    await page.evaluate(() => document.fonts.ready);

    // Shell geometry — identical on all five screens per the Soft Bento spec.
    const canvas = await page.locator(".app-canvas").first();
    const canvasBox = await canvas.boundingBox();
    eq(`${id}: canvas width`, Math.round(canvasBox?.width ?? -1), VIEWPORT.width);
    eq(`${id}: canvas height`, Math.round(canvasBox?.height ?? -1), VIEWPORT.height);
    const windowBox = await page.locator(".app-window").first().boundingBox();
    eq(`${id}: window left inset`, Math.round(windowBox?.x ?? -1), PEN.canvasPadding);
    eq(`${id}: window top inset`, Math.round(windowBox?.y ?? -1), PEN.canvasPadding);
    eq(`${id}: window width`, Math.round(windowBox?.width ?? -1), VIEWPORT.width - PEN.canvasPadding * 2);
    eq(`${id}: window height`, Math.round(windowBox?.height ?? -1), VIEWPORT.height - PEN.canvasPadding * 2);
    const bar = await page.locator(".window-bar").first().boundingBox();
    eq(`${id}: window bar height`, Math.round(bar?.height ?? -1), PEN.windowBarHeight);
    const rail = await page.locator("nav[role=navigation]").first();
    eq(`${id}: sidebar width`, Math.round((await rail.boundingBox())?.width ?? -1), PEN.sidebarWidth);
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

    const windowBg = await page.evaluate(() => getComputedStyle(document.querySelector(".app-window")).backgroundColor);
    eq(`${id}: window background`, windowBg, PEN.windowBackground);

    const bodyFont = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
    matches(`${id}: body font`, bodyFont, PEN.fontFamily);

    // The active sidebar item is the only filled navigation state.
    const activeSurface = await page.evaluate(() => {
      const el = document.querySelector('nav[role=navigation] [role=tab][aria-selected="true"]');
      return el ? getComputedStyle(el).backgroundColor : null;
    });
    eq(`${id}: active nav surface`, activeSurface, PEN.activeSurface);

    // Screen-specific anchor.
    if (id === "home") {
      eq("home: stat card count", await page.locator("main .home-stat-card").count(), PEN.homeStatCount);
    }

    await page.screenshot({ path: join(OUT, `${id}.png`) });
    ok(`${id}: captured → docs/design/actual/${id}.png`);
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
