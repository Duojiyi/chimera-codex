#!/usr/bin/env node
// Build `chimera-app-latest.json` — the document a Chimera client reads to
// decide whether, and to what, it should update itself.
//
// Separate from scripts/release-manifest.mjs, which serves 1.x: that one
// requires the `X.Y.Z-chimera.N` version format and `ChimeraPlusPlus-` asset
// names, and 1.x releases still depend on both. Bending it to also serve v2
// would couple two release trains that deliberately diverged.
//
// This file is a TARGET, not a trust root. Per ADR-006 and G8, a client must
// only act on it after the app-domain metadata chain has pinned its digest —
// a signature on this document alone would let a replayed older copy pass.
// scripts/sign-manifest.mjs attaches that signature; chimera-update verifies
// the chain.
//
// Usage:
//   node scripts/v2-release-manifest.mjs --dist <dir> --out <file> [--self-test]
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, readdirSync, statSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { join, relative, basename } from "node:path";
import { tmpdir } from "node:os";

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?$/;

/** Assets a client could install. Checksums and notices are not installable. */
const INSTALLABLE = /\.(exe|dmg|zip|tar\.gz|AppImage)$/i;

export function parseVersion(value, label = "version") {
  if (typeof value !== "string") throw new Error(`${label} must be a string`);
  const normalized = value.replace(/^[vV]/, "");
  const m = SEMVER.exec(normalized);
  if (!m) throw new Error(`Invalid ${label}: ${value}`);
  return { value: normalized, parts: [Number(m[1]), Number(m[2]), Number(m[3])], pre: m[4] ?? null };
}

/** Negative when a < b. */
export function compareVersions(a, b) {
  const x = parseVersion(a), y = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    if (x.parts[i] !== y.parts[i]) return x.parts[i] - y.parts[i];
  }
  // A prerelease sorts below the release it precedes (2.0.0-beta < 2.0.0).
  if (x.pre === y.pre) return 0;
  if (x.pre === null) return 1;
  if (y.pre === null) return -1;
  return x.pre < y.pre ? -1 : 1;
}

/**
 * Detect the platform and architecture an asset targets from its filename.
 *
 * Returns null when neither can be determined, and the caller refuses the
 * asset rather than guessing: an asset published under the wrong platform
 * would be offered to machines that cannot run it, and the client has no way
 * to notice before downloading it.
 */
export function classifyAsset(name) {
  const lower = name.toLowerCase();
  const platform = lower.includes("windows") || lower.endsWith(".exe")
    ? "windows"
    : lower.includes("macos") || lower.includes("darwin") || lower.endsWith(".dmg")
      ? "macos"
      : null;
  const arch = /\b(arm64|aarch64)\b/.test(lower)
    ? "arm64"
    : /\b(x64|x86_64|amd64)\b/.test(lower)
      ? "x64"
      : null;
  if (!platform || !arch) return null;
  return { platform, arch };
}

/**
 * @param {{distDir: string, version: string, repository: string, tag: string,
 *          minimumSupportedVersion?: string}} opts
 */
export function buildManifest({ distDir, version, repository, tag, minimumSupportedVersion }) {
  const v = parseVersion(version);
  const floor = parseVersion(minimumSupportedVersion || v.value, "minimum_supported_version");
  if (compareVersions(floor.value, v.value) > 0) {
    throw new Error(`minimum_supported_version ${floor.value} exceeds release ${v.value}`);
  }

  const files = [];
  const walk = (dir) => {
    for (const e of readdirSync(dir)) {
      const full = join(dir, e);
      if (statSync(full).isDirectory()) walk(full);
      else files.push(full);
    }
  };
  walk(distDir);

  const assets = [];
  const skipped = [];
  for (const full of files.sort()) {
    const name = basename(full);
    if (!INSTALLABLE.test(name)) continue;
    const cls = classifyAsset(name);
    if (!cls) {
      // Loudly, not silently: an asset the manifest drops is one users never
      // receive, and a silent drop looks identical to a successful release.
      skipped.push(relative(distDir, full));
      continue;
    }
    const bytes = readFileSync(full);
    assets.push({
      name,
      platform: cls.platform,
      arch: cls.arch,
      url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(name)}`,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      size: bytes.length,
    });
  }

  if (skipped.length > 0) {
    throw new Error(
      `could not determine platform/arch for: ${skipped.join(", ")}. ` +
        `Rename them to include both (for example "-windows-x64-") rather than ` +
        `publishing a release that silently omits them.`,
    );
  }
  if (assets.length === 0) {
    throw new Error(`no installable assets found under ${distDir}`);
  }

  return {
    schema_version: 1,
    version: v.value,
    minimum_supported_version: floor.value,
    // Stated in the artifact itself so a client, a mirror or a user reading the
    // JSON sees the signing posture without having to find ADR-008.
    signing: {
      os_code_signing: "none",
      note: "Windows builds are unsigned (SmartScreen will warn). macOS builds are ad-hoc signed and not notarized. Manifest integrity is provided by the app-domain metadata chain, not by OS code signing. See ADR-008.",
    },
    release_url: `https://github.com/${repository}/releases/tag/${tag}`,
    assets,
  };
}

// ── Self-test ──────────────────────────────────────────────────────────────

function selfTest() {
  const dir = mkdtempSync(join(tmpdir(), "chimera-v2-manifest-"));
  let failures = 0;
  const check = (label, fn) => {
    try {
      fn();
      console.log(`\x1b[32m✓\x1b[0m ${label}`);
    } catch (e) {
      console.log(`\x1b[31m✗\x1b[0m ${label}: ${e.message}`);
      failures++;
    }
  };
  const expectThrow = (label, fn, pattern) =>
    check(label, () => {
      let threw = null;
      try { fn(); } catch (e) { threw = e; }
      if (!threw) throw new Error("expected a rejection, got success");
      if (!pattern.test(threw.message)) throw new Error(`wrong error: ${threw.message}`);
    });

  try {
    writeFileSync(join(dir, "Chimera++_2.0.0_windows-x64-setup.exe"), "win");
    writeFileSync(join(dir, "Chimera++_2.0.0_macos-arm64.dmg"), "mac");
    writeFileSync(join(dir, "checksums.txt"), "not installable");
    const base = { distDir: dir, version: "2.0.0", repository: "Duojiyi/chimera-codex", tag: "v2.0.0" };

    check("builds a manifest from a well-named dist tree", () => {
      const m = buildManifest(base);
      if (m.assets.length !== 2) throw new Error(`expected 2 assets, got ${m.assets.length}`);
      if (!m.assets.every((a) => /^[0-9a-f]{64}$/.test(a.sha256))) throw new Error("bad digest");
      if (m.assets.some((a) => a.size === 0)) throw new Error("zero-size asset");
    });

    check("non-installable files are not offered as updates", () => {
      const m = buildManifest(base);
      if (m.assets.some((a) => a.name === "checksums.txt")) throw new Error("checksums.txt was listed");
    });

    expectThrow(
      "refuses an asset whose platform/arch cannot be determined",
      () => {
        writeFileSync(join(dir, "Chimera++_2.0.0_setup.exe"), "ambiguous");
        try { return buildManifest(base); } finally { rmSync(join(dir, "Chimera++_2.0.0_setup.exe")); }
      },
      /could not determine platform\/arch/,
    );

    expectThrow(
      "refuses a floor above the release itself",
      () => buildManifest({ ...base, minimumSupportedVersion: "2.1.0" }),
      /exceeds release/,
    );

    expectThrow("refuses a malformed version", () => buildManifest({ ...base, version: "two" }), /Invalid version/);

    check("prerelease sorts below its release", () => {
      if (compareVersions("2.0.0-beta", "2.0.0") >= 0) throw new Error("2.0.0-beta must sort below 2.0.0");
      if (compareVersions("2.0.0", "2.0.1") >= 0) throw new Error("2.0.0 must sort below 2.0.1");
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  console.log(failures === 0 ? "\nv2-release-manifest self-test: PASS" : `\nv2-release-manifest self-test: ${failures} failure(s)`);
  process.exit(failures > 0 ? 1 : 0);
}

// ── CLI ────────────────────────────────────────────────────────────────────

const argv = process.argv.slice(2);
const flag = (name) => {
  const i = argv.indexOf(name);
  return i === -1 ? null : argv[i + 1];
};

if (argv.includes("--self-test")) selfTest();
else {
  const dist = flag("--dist");
  const out = flag("--out");
  if (!dist || !out) {
    console.error("usage: v2-release-manifest.mjs --dist <dir> --out <file> [--self-test]");
    process.exit(2);
  }
  if (!existsSync(dist)) {
    console.error(`--dist directory not found: ${dist}`);
    process.exit(1);
  }
  const tag = process.env.TAG || process.env.GITHUB_REF_NAME;
  if (!tag) {
    console.error("TAG (or GITHUB_REF_NAME) must be set — asset URLs are built from it");
    process.exit(1);
  }
  const manifest = buildManifest({
    distDir: dist,
    version: process.env.VERSION || tag,
    repository: process.env.REPO || process.env.GITHUB_REPOSITORY,
    tag,
    minimumSupportedVersion: process.env.MINIMUM_SUPPORTED_VERSION || "",
  });
  writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`wrote ${out} (${manifest.assets.length} assets, version ${manifest.version})`);
}
