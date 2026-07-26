#!/usr/bin/env node
// V9 — Mirror contract verification (cross-platform, Node built-ins only)
// Validates manifest schema, stable CAS rules, capability binding, and that
// raw channel cannot bypass stability gate to reach end users.
// Usage: node scripts/test-mirror-contract.mjs [--fixture <path>]
import { readFileSync, existsSync, readdirSync } from "fs";
import { join } from "path";
import { fileURLToPath } from "url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
const PASS = "\x1b[32m✓\x1b[0m";
const FAIL = "\x1b[31m✗\x1b[0m";
let failures = 0;

function check(label, ok, detail = "") {
  console.log(`${ok ? PASS : FAIL} ${label}${detail ? `: ${detail}` : ""}`);
  if (!ok) failures++;
}

// ── Required manifest fields ──────────────────────────────────────────────────

const REQUIRED_MANIFEST_FIELDS = [
  "schema_version", "channel", "codex_version", "published_at",
  "platform", "arch", "asset_url", "size_bytes", "sha256",
  "official_identity", "minimum_chimera_version", "compatibility_status",
  "source_provenance",
];

function validateManifest(manifest, name) {
  for (const field of REQUIRED_MANIFEST_FIELDS) {
    check(`${name}: has required field '${field}'`, field in manifest);
  }
  check(`${name}: schema_version >= 1`, manifest.schema_version >= 1);
  check(`${name}: sha256 is 64 hex chars`, /^[0-9a-f]{64}$/i.test(manifest.sha256));
  check(`${name}: channel is raw|stable|candidate`,
    ["raw", "stable", "candidate"].includes(manifest.channel));
  check(`${name}: asset_url is https`, (manifest.asset_url ?? "").startsWith("https://"));

  // Spec 9.2: a stable entry must name the capability manifest generated for
  // the same raw build, with size and digest so it can be verified BEFORE use.
  // All three are required together — a URL alone cannot be checked.
  if (manifest.channel === "stable") {
    check(
      `${name}: stable declares capability_manifest_url`,
      typeof manifest.capability_manifest_url === "string" &&
        manifest.capability_manifest_url.startsWith("https://"),
    );
    check(
      `${name}: stable declares capability_manifest_size_bytes > 0`,
      Number.isInteger(manifest.capability_manifest_size_bytes) &&
        manifest.capability_manifest_size_bytes > 0,
    );
    check(
      `${name}: stable declares capability_manifest_sha256 (64 hex)`,
      /^[0-9a-f]{64}$/i.test(manifest.capability_manifest_sha256 ?? ""),
    );
  }
  check(`${name}: size_bytes > 0`, (manifest.size_bytes ?? 0) > 0);

  // Raw channel must NOT have compatibility_status = "compatible"
  if (manifest.channel === "raw") {
    const cs = manifest.compatibility_status;
    const isCompat = cs === "compatible" || cs?.status === "compatible";
    check(`${name}: raw channel must NOT have compatibility_status=compatible`, !isCompat);
  }

  // Stable channel MUST have compatibility_status = "compatible"
  if (manifest.channel === "stable") {
    const cs = manifest.compatibility_status;
    const isCompat = cs === "compatible" || cs?.status === "compatible";
    check(`${name}: stable channel must have compatibility_status=compatible`, isCompat);
  }
}

// ── CAS sequence check ────────────────────────────────────────────────────────

function validateCasSequence(pointers) {
  if (pointers.length < 2) return;
  for (let i = 1; i < pointers.length; i++) {
    const prev = pointers[i - 1];
    const curr = pointers[i];
    check(
      `CAS: sequence ${prev.sequence} → ${curr.sequence} is monotonically increasing`,
      curr.sequence > prev.sequence,
    );
  }
}

// ── Capability manifest binding ───────────────────────────────────────────────

function validateCapabilityBinding(capManifest, stableManifest, name) {
  check(
    `${name}: capability bound_raw_digest matches stable promoted_from_raw_digest`,
    capManifest.bound_raw_digest === stableManifest.promoted_from_raw_digest,
    `cap=${capManifest.bound_raw_digest} stable=${stableManifest.promoted_from_raw_digest}`,
  );
  check(`${name}: capability has skin_compat field`, "skin_compat" in capManifest);
}

// ── Load and check fixtures ───────────────────────────────────────────────────

const fixturesDir = join(ROOT, "services", "mirror-contract", "fixtures");

if (!existsSync(fixturesDir)) {
  console.log(`\nℹ No fixtures directory found at services/mirror-contract/fixtures/ — running structural checks only.`);
} else {
  const files = readdirSync(fixturesDir).filter(f => f.endsWith(".json"));
  if (files.length === 0) {
    console.log(`\nℹ No fixture JSON files found — running structural checks only.`);
  } else {
    for (const file of files) {
      const content = JSON.parse(readFileSync(join(fixturesDir, file), "utf8"));
      if (Array.isArray(content)) {
        content.forEach((m, i) => validateManifest(m, `${file}[${i}]`));
      } else if (content.type === "stable_pointer_history") {
        validateCasSequence(content.pointers ?? []);
      } else if (content.type === "capability_binding_test") {
        validateCapabilityBinding(content.cap_manifest, content.stable_manifest, file);
      } else {
        validateManifest(content, file);
      }
    }
  }
}

// ── Inline contract invariant checks (no fixtures needed) ─────────────────────

// These run unconditionally to verify the schema logic itself.

// 1. Raw manifest must never have compatible status
const rawManifest = {
  schema_version: 1, channel: "raw", codex_version: "26.721",
  published_at: "2026-07-26T00:00:00Z", platform: "windows", arch: "x64",
  asset_url: "https://example.com/payload.msix", size_bytes: 100_000_000,
  sha256: "a".repeat(64), official_identity: { signer: "test" },
  minimum_chimera_version: "2.0.0", compatibility_status: "pending",
  source_provenance: { source_url: "https://src.example.com", observed_at: "2026-07-26T00:00:00Z" },
};
validateManifest(rawManifest, "inline-raw");

// 2. Stable manifest must have compatible status AND a complete capability
//    triple (Spec 9.2): a stable entry that names no capability manifest would
//    let the skin engine run against a build whose capabilities are unknown.
const stableManifest = {
  ...rawManifest, channel: "stable",
  compatibility_status: "compatible",
  promoted_from_raw_digest: "sha256:" + "b".repeat(60),
  capability_manifest_url: "https://example.com/capability-26.721.json",
  capability_manifest_size_bytes: 512,
  capability_manifest_sha256: "c".repeat(64),
};
validateManifest(stableManifest, "inline-stable");

// 3. CAS sequence monotonicity
validateCasSequence([
  { sequence: 1 }, { sequence: 2 }, { sequence: 3 },
]);

// 4. Self-test: the capability requirement must actually bite. A gate that
//    only ever passes proves nothing, so drop each field in turn and confirm
//    the validator rejects it. Failures raised here are expected and are not
//    counted against the run.
const CAPABILITY_FIELDS = [
  "capability_manifest_url",
  "capability_manifest_size_bytes",
  "capability_manifest_sha256",
];
let selfTestBroken = 0;
for (const field of CAPABILITY_FIELDS) {
  const incomplete = { ...stableManifest };
  delete incomplete[field];

  const before = failures;
  const realLog = console.log;
  console.log = () => {}; // the expected failure output would be noise
  validateManifest(incomplete, `selftest-missing-${field}`);
  console.log = realLog;

  const raised = failures - before;
  failures = before; // these failures were expected; do not fail the run
  if (raised === 0) {
    selfTestBroken += 1;
    failures += 1;
    console.log(
      `${FAIL} self-test: a stable manifest missing ${field} was accepted — ` +
        `the capability requirement is not enforced`,
    );
  }
}
if (selfTestBroken === 0) {
  console.log(
    `${PASS} self-test: dropping any of the 3 capability fields is rejected`,
  );
}

// ── Summary ───────────────────────────────────────────────────────────────────

console.log(`\n${failures === 0 ? PASS : FAIL} test-mirror-contract: ${failures === 0 ? "PASS" : `${failures} failure(s)`}`);
process.exit(failures > 0 ? 1 : 0);
