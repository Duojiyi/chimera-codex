/**
 * Integration test — Bug 3: CSS scrollbar fix + update banner styles.
 *
 * Reads src/chimera.css directly and asserts:
 *  - .route-line-scroll uses `scrollbar-width: none` (not `thin`)
 *  - webkit scrollbar is hidden via display:none
 *  - .route-line-card uses the shrinkable flex shorthand (1 1 160px)
 *  - .route-update-banner block is present (Bug 2 styles)
 */
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const CSS_PATH = path.resolve(__dirname, "../../src/chimera.css");
const css = fs.readFileSync(CSS_PATH, "utf8");

// ---------------------------------------------------------------------------
// Helper: extract the text of the FIRST CSS block whose selector matches
// ---------------------------------------------------------------------------
function extractBlock(selector: string): string {
  // Escape selector for regex use
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "s");
  const match = re.exec(css);
  return match ? match[1] : "";
}

describe("chimera.css — Bug 3 scrollbar fix", () => {
  it(".route-line-scroll hides the native scrollbar via scrollbar-width: none", () => {
    const block = extractBlock(".route-line-scroll");
    expect(block).toContain("scrollbar-width: none");
  });

  it(".route-line-scroll does NOT use scrollbar-width: thin", () => {
    const block = extractBlock(".route-line-scroll");
    expect(block).not.toContain("scrollbar-width: thin");
  });

  it(".route-line-scroll hides webkit scrollbar via -ms-overflow-style: none", () => {
    const block = extractBlock(".route-line-scroll");
    expect(block).toContain("-ms-overflow-style: none");
  });

  it("::-webkit-scrollbar rule has display: none to hide it in Chrome/Edge", () => {
    // This rule appears OUTSIDE the .route-line-scroll block
    expect(css).toMatch(/\.route-line-scroll::-webkit-scrollbar\s*\{[^}]*display:\s*none/s);
  });

  it(".route-line-card uses shrinkable flex (1 1 160px), not the rigid 1 0 260px", () => {
    const block = extractBlock(".route-line-card");
    // The value we fixed to — allows cards to shrink when container is narrow
    expect(block).toMatch(/flex:\s*1\s+1\s+160px/);
    expect(block).not.toMatch(/flex:\s*1\s+0\s+260px/);
  });
});

describe("chimera.css — Bug 2 update banner styles", () => {
  it(".route-update-banner block exists", () => {
    expect(css).toContain(".route-update-banner");
  });

  it(".route-update-banner-copy block exists", () => {
    expect(css).toContain(".route-update-banner-copy");
  });

  it(".route-update-banner-actions block exists", () => {
    expect(css).toContain(".route-update-banner-actions");
  });

  it(".route-update-banner-actions button.primary block exists", () => {
    expect(css).toContain("button.primary");
  });
});
