// WCAG 2.2 AA gate — the real implementation behind `npm run test:a11y`.
//
// G16: core UI must reach WCAG 2.2 AA with zero axe serious/critical findings.
// Runs axe-core inside a real Chromium page, because the rules that matter most
// here (colour-contrast against our near-black palette, focus visibility) need
// real computed styles. A jsdom shim would silently skip exactly those rules.
//
// Usage:  node a11y.mjs [--lang zh|en]
// Exit 0 = zero serious/critical findings. Exit 1 = gate fails.

import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const APP = resolve(HERE, "../../apps/chimera-desktop");
const AXE_SOURCE = readFileSync(
  join(HERE, "node_modules/axe-core/axe.min.js"),
  "utf8",
);

const PORT = 4174;
const BASE = `http://127.0.0.1:${PORT}`;
const SCREENS = ["home", "providers", "codex", "appearance", "settings"];

// WCAG 2.2 AA and everything it subsumes.
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

const langArg = process.argv.indexOf("--lang");
const LANG = langArg !== -1 ? process.argv[langArg + 1] : "zh";

const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const RESET = "\x1b[0m";

function startPreview() {
  const proc = spawn("npm", ["run", "vite:preview", "--", "--port", String(PORT)], {
    cwd: APP,
    shell: true,
    stdio: "ignore",
  });
  return proc;
}

async function waitForServer(timeoutMs = 45000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(BASE);
      if (res.ok) return true;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 400));
  }
  return false;
}

const server = startPreview();
let failed = 0;
let seriousCritical = 0;

try {
  if (!(await waitForServer())) {
    console.error(`${RED}could not start preview server on ${PORT}${RESET}`);
    console.error("hint: run `npm run vite:build` in apps/chimera-desktop first");
    process.exit(1);
  }

  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });

  for (const screen of SCREENS) {
    await page.goto(`${BASE}/?screen=${screen}&lang=${LANG}`, {
      waitUntil: "networkidle",
    });
    await page.addScriptTag({ content: AXE_SOURCE });

    const result = await page.evaluate(
      async (tags) =>
        await window.axe.run(document, {
          runOnly: { type: "tag", values: tags },
        }),
      TAGS,
    );

    const blocking = result.violations.filter(
      (v) => v.impact === "serious" || v.impact === "critical",
    );
    const minor = result.violations.filter(
      (v) => v.impact !== "serious" && v.impact !== "critical",
    );

    if (blocking.length === 0) {
      console.log(
        `${GREEN}✓${RESET} ${screen} (${LANG}): no serious/critical` +
          (minor.length ? ` ${YELLOW}(${minor.length} minor/moderate)${RESET}` : ""),
      );
    } else {
      failed++;
      seriousCritical += blocking.length;
      console.log(`${RED}✗${RESET} ${screen} (${LANG}): ${blocking.length} serious/critical`);
      for (const v of blocking) {
        console.log(`    [${v.impact}] ${v.id} — ${v.help}`);
        for (const node of v.nodes.slice(0, 3)) {
          console.log(`      ${node.target.join(" ")}`);
          if (node.failureSummary) {
            const first = node.failureSummary.split("\n").filter(Boolean)[1];
            if (first) console.log(`        ${first.trim()}`);
          }
        }
        if (v.nodes.length > 3) {
          console.log(`      … and ${v.nodes.length - 3} more node(s)`);
        }
      }
    }

    for (const v of minor) {
      console.log(`  ${YELLOW}·${RESET} ${screen}: [${v.impact}] ${v.id} — ${v.help}`);
    }
  }

  await browser.close();
} finally {
  server.kill();
}

console.log("");
if (failed === 0) {
  console.log(`${GREEN}✓ test:a11y (${LANG}): WCAG 2.2 AA — zero serious/critical${RESET}`);
  process.exit(0);
} else {
  console.log(
    `${RED}✗ test:a11y (${LANG}): ${seriousCritical} serious/critical finding(s) on ${failed} screen(s)${RESET}`,
  );
  process.exit(1);
}
