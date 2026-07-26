#!/usr/bin/env node
// V10 — Bundle contract (cross-platform, Node built-ins only).
//
// What ships to a user, and what must never ship.
//
// D6 was revised on 2026-07-26: no official Codex binary is distributed with
// our package. The client downloads the payload on first run and verifies it
// against a signed stable manifest. That turns "the bundle contains the
// payload" from a requirement into a stop condition, and this gate is what
// enforces the inversion — a build that quietly regains a payload would
// otherwise re-open the legal review D6 was changed to avoid.
//
// ADR-008 additionally decided the bundle is unsigned (NSIS on Windows, ad-hoc
// on macOS). That makes the checks here the only automated statement about
// what is inside it.
//
// Usage:
//   node scripts/test-bundle-contract.mjs             structural + self-test
//   node scripts/test-bundle-contract.mjs --dist <d>  also scan a built tree
import { readFileSync, existsSync, readdirSync, statSync } from "fs";
import { join, relative, basename, extname } from "path";
import { fileURLToPath } from "url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const PASS = "\x1b[32m✓\x1b[0m";
const FAIL = "\x1b[31m✗\x1b[0m";
let failures = 0;

function check(label, ok, detail = "") {
  console.log(`${ok ? PASS : FAIL} ${label}${detail ? `: ${detail}` : ""}`);
  if (!ok) failures++;
}

// ── What must never be in a bundle ─────────────────────────────────────────

/**
 * Files that would mean we are redistributing the official Codex payload.
 *
 * Matched on the basename, case-insensitively, because a bundle that carried
 * `payload/Codex.exe` is no better than one carrying it at the root. The MSIX
 * and APPX extensions are here because that is the shape the official build
 * ships in, and an unpacked copy is still a copy.
 */
const FORBIDDEN_PAYLOAD = [
  /^codex\.exe$/i,
  /^codex$/i,
  /^chatgpt\.exe$/i,
  /^codex\.app$/i,
];
const FORBIDDEN_PAYLOAD_EXT = [".msix", ".msixbundle", ".appx", ".appxbundle"];

/**
 * Content patterns that mean a credential leaked into the package.
 *
 * Deliberately broader than the repository secret scan (V12): that one runs on
 * source, where a false positive is cheap to inspect. This one runs on the
 * artifact a user downloads, where a false negative is unrecoverable — the key
 * is already published.
 */
const SECRET_PATTERNS = [
  [/\bsk-[A-Za-z0-9]{20,}\b/, "OpenAI-style API key"],
  [/\bghp_[A-Za-z0-9]{30,}\b/, "GitHub personal access token"],
  [/\bgithub_pat_[A-Za-z0-9_]{50,}\b/, "GitHub fine-grained token"],
  [/\bAKIA[0-9A-Z]{16}\b/, "AWS access key id"],
  [/\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/, "JWT"],
  [/https?:\/\/[^\s/:@]+:[^\s/@]+@/, "URL with embedded credentials"],
  [/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/, "private key"],
];

/** Files every delivered bundle must carry. */
const REQUIRED_FILES = ["NOTICE", "LICENSE"];

/**
 * Judge a bundle from its file list plus a reader for text content.
 *
 * Pure apart from `readText`, so the self-tests below can drive it with
 * synthetic bundles and prove each rule actually rejects.
 *
 * @param {{path: string, size: number}[]} entries
 * @param {(path: string) => string | null} readText  null for binary/unreadable
 * @returns {string[]} one message per violation
 */
export function bundleViolations(entries, readText = () => null) {
  const violations = [];

  for (const entry of entries) {
    const name = basename(entry.path);
    const ext = extname(name).toLowerCase();

    if (FORBIDDEN_PAYLOAD.some((re) => re.test(name))) {
      violations.push(
        `${entry.path}: looks like the official Codex payload. D6 forbids shipping it; ` +
          `the client downloads and verifies it on first run.`,
      );
    }
    if (FORBIDDEN_PAYLOAD_EXT.includes(ext)) {
      violations.push(
        `${entry.path}: an ${ext} package is the official payload's distribution format ` +
          `and must not be bundled (D6).`,
      );
    }

    const text = readText(entry.path);
    if (text === null) continue;
    for (const [re, what] of SECRET_PATTERNS) {
      if (re.test(text)) {
        // Never echo the match itself — this output goes to CI logs.
        violations.push(`${entry.path}: contains what looks like a ${what} (G4).`);
        break;
      }
    }
  }

  const names = new Set(entries.map((e) => basename(e.path).toUpperCase()));
  for (const required of REQUIRED_FILES) {
    if (!names.has(required.toUpperCase())) {
      violations.push(`missing required file: ${required}`);
    }
  }

  return violations;
}

// ── Self-test: prove each rule can reject ──────────────────────────────────
// A gate nobody has seen go red is indistinguishable from one that cannot.

const CLEAN_BUNDLE = [
  { path: "Chimera++.exe", size: 12_000_000 },
  { path: "NOTICE", size: 4_000 },
  { path: "LICENSE", size: 34_000 },
  { path: "checksums.txt", size: 300 },
];

{
  const clean = bundleViolations(CLEAN_BUNDLE);
  check("self-test: a clean bundle passes", clean.length === 0, clean.join("; "));
}

const REJECTION_CASES = [
  [
    "a bundled Codex.exe",
    [...CLEAN_BUNDLE, { path: "payload/Codex.exe", size: 90_000_000 }],
    () => null,
  ],
  [
    "a bundled .msix payload",
    [...CLEAN_BUNDLE, { path: "assets/Codex_26.721_x64.msix", size: 120_000_000 }],
    () => null,
  ],
  [
    "a Codex binary nested in a subdirectory",
    [...CLEAN_BUNDLE, { path: "resources/bin/codex", size: 80_000_000 }],
    () => null,
  ],
  [
    "an API key in a shipped config",
    [...CLEAN_BUNDLE, { path: "config.json", size: 200 }],
    (p) => (p === "config.json" ? '{"key":"sk-abcdefghijklmnopqrstuvwxyz012345"}' : null),
  ],
  [
    "a URL with embedded credentials",
    [...CLEAN_BUNDLE, { path: "settings.ini", size: 120 }],
    (p) => (p === "settings.ini" ? "endpoint=https://user:hunter2@api.example.com/v1" : null),
  ],
  [
    "a private key",
    [...CLEAN_BUNDLE, { path: "keys/signing.pem", size: 1700 }],
    (p) => (p.endsWith(".pem") ? "-----BEGIN PRIVATE KEY-----\nMIIE...\n" : null),
  ],
  ["a bundle with no NOTICE", CLEAN_BUNDLE.filter((e) => e.path !== "NOTICE"), () => null],
];

for (const [label, entries, reader] of REJECTION_CASES) {
  const found = bundleViolations(entries, reader);
  check(`self-test: rejects ${label}`, found.length > 0);
}

// ── Release workflow must not reintroduce a payload ────────────────────────
// The gate above judges a bundle that exists. This one judges the recipe, so a
// payload-fetching step is caught at review time rather than at release time.

{
  const wf = join(ROOT, ".github", "workflows", "v2-release.yml");
  if (!existsSync(wf)) {
    console.log("ℹ v2-release.yml not present yet — skipping recipe check (Step 6.2)");
  } else {
    const text = readFileSync(wf, "utf8");
    // Downloading the payload *in the release job* is what would put it in the
    // artifact. The client downloading it at runtime is the whole design.
    const suspicious = [
      [/msix/i, "references an MSIX package"],
      [/download.*codex.*(payload|binary|msix)/i, "downloads a Codex payload"],
    ].filter(([re]) => re.test(text));
    check(
      "release workflow does not fetch a Codex payload into the artifact",
      suspicious.length === 0,
      suspicious.map(([, w]) => w).join("; "),
    );
  }
}

// ── Optional: scan a real built tree ───────────────────────────────────────

const distFlag = process.argv.indexOf("--dist");
if (distFlag !== -1) {
  const dist = process.argv[distFlag + 1];
  if (!dist || !existsSync(dist)) {
    check(`--dist ${dist ?? "(missing)"} exists`, false);
  } else {
    const entries = [];
    const walk = (dir) => {
      for (const e of readdirSync(dir)) {
        const full = join(dir, e);
        const st = statSync(full);
        if (st.isDirectory()) walk(full);
        else entries.push({ path: relative(dist, full).split("\\").join("/"), size: st.size });
      }
    };
    walk(dist);

    // Only read files small and plausible enough to be text; a 100 MB binary is
    // not worth regexing and would dominate the run.
    const readText = (p) => {
      const full = join(dist, p);
      try {
        if (statSync(full).size > 2_000_000) return null;
        const buf = readFileSync(full);
        if (buf.includes(0)) return null; // NUL byte: treat as binary
        return buf.toString("utf8");
      } catch {
        return null;
      }
    };

    const violations = bundleViolations(entries, readText);
    if (violations.length === 0) {
      check(`bundle at ${dist} satisfies the contract (${entries.length} files)`, true);
    } else {
      for (const v of violations) check(`bundle: ${v}`, false);
    }
  }
} else {
  console.log("ℹ no --dist given — structural and self-test checks only");
}

console.log(
  `\n${failures === 0 ? PASS : FAIL} test-bundle-contract: ${failures === 0 ? "PASS" : `${failures} failure(s)`}`,
);
process.exit(failures > 0 ? 1 : 0);
