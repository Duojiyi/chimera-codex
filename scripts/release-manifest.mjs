import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const CHIMERA_VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-chimera\.(0|[1-9]\d*)$/;
const U64_MAX = 18_446_744_073_709_551_615n;

export function normalizeChimeraVersion(value, label) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be a string`);
  }
  const normalized = value.replace(/^[vV]/, "");
  const match = CHIMERA_VERSION_PATTERN.exec(normalized);
  if (!match) {
    throw new Error(`Invalid ${label}: ${value}`);
  }
  const parts = match.slice(1).map((part) => BigInt(part));
  if (parts.some((part) => part > U64_MAX)) {
    throw new Error(`${label} contains a component outside u64 range: ${value}`);
  }
  return {
    value: normalized,
    parts,
  };
}

function compareVersionParts(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] < right[index]) return -1;
    if (left[index] > right[index]) return 1;
  }
  return 0;
}

export function resolveMinimumSupportedVersion(version, configuredMinimum) {
  const latest = normalizeChimeraVersion(version, "VERSION");
  const requested = configuredMinimum || latest.value;
  const minimum = normalizeChimeraVersion(requested, "MINIMUM_SUPPORTED_VERSION");
  if (compareVersionParts(minimum.parts, latest.parts) > 0) {
    throw new Error(
      `MINIMUM_SUPPORTED_VERSION ${minimum.value} exceeds release ${latest.value}`,
    );
  }
  return { version: latest.value, minimumSupportedVersion: minimum.value };
}

export function validateManifestFloor(payload, expectedMinimum = "") {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new Error("latest manifest must be an object");
  }
  if (!Object.hasOwn(payload, "minimum_supported_version")) {
    throw new Error("latest manifest missing minimum_supported_version");
  }
  if (typeof payload.minimum_supported_version !== "string") {
    throw new Error("minimum_supported_version must be a string");
  }
  const latest = normalizeChimeraVersion(payload.version, "manifest version");
  const minimum = normalizeChimeraVersion(
    payload.minimum_supported_version,
    "minimum_supported_version",
  );
  if (compareVersionParts(minimum.parts, latest.parts) > 0) {
    throw new Error(
      `minimum_supported_version ${minimum.value} exceeds release ${latest.value}`,
    );
  }
  const resolved = {
    version: latest.value,
    minimumSupportedVersion: minimum.value,
  };
  if (expectedMinimum) {
    const expected = resolveMinimumSupportedVersion(payload.version, expectedMinimum);
    if (resolved.minimumSupportedVersion !== expected.minimumSupportedVersion) {
      throw new Error(
        `latest manifest floor ${resolved.minimumSupportedVersion} does not match expected ${expected.minimumSupportedVersion}`,
      );
    }
  }
  return resolved;
}

export function generateManifest({
  repository,
  version,
  tag,
  minimumSupportedVersion,
  assetDirectory,
  writeFiles = true,
}) {
  if (!repository || !tag) {
    throw new Error("REPO and TAG are required");
  }
  const resolved = resolveMinimumSupportedVersion(version, minimumSupportedVersion);
  const baseUrl = `https://github.com/${repository}/releases/download/${tag}`;
  const files = fs
    .readdirSync(assetDirectory)
    .filter(
      (name) =>
        name.startsWith("ChimeraPlusPlus-") &&
        name !== `ChimeraPlusPlus-${resolved.version}-source.tar.gz`,
    )
    .sort();

  const assets = files.map((name) => {
    const fullPath = path.join(assetDirectory, name);
    const bytes = fs.readFileSync(fullPath);
    return {
      name,
      url: `${baseUrl}/${encodeURIComponent(name)}`,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
      size: bytes.length,
    };
  });

  const payload = {
    version: `v${resolved.version}`,
    minimum_supported_version: `v${resolved.minimumSupportedVersion}`,
    url: `https://github.com/${repository}/releases/tag/${tag}`,
    body: [
      `Chimera++ ${resolved.version}`,
      "",
      "License: AGPL-3.0-only; third-party notices: NOTICE",
      `Corresponding source: ${baseUrl}/ChimeraPlusPlus-${resolved.version}-source.tar.gz`,
      "",
      "macOS builds are ad-hoc signed only and are **not notarized**.",
      "On first launch, use right-click -> Open (or clear quarantine) as documented in README.",
    ].join("\n"),
    assets,
  };

  validateManifestFloor(payload, resolved.minimumSupportedVersion);
  if (writeFiles) {
    fs.writeFileSync("latest.json", `${JSON.stringify(payload, null, 2)}\n`);
    fs.writeFileSync("release-notes.md", `${payload.body}\n`);
  }
  return payload;
}

function assertThrows(action, pattern) {
  assert.throws(action, pattern);
}

export function selfTest() {
  const fixtureDir = fs.mkdtempSync(path.join(os.tmpdir(), "chimera-release-manifest-"));
  const fixtureName = "ChimeraPlusPlus-1.2.35-chimera.1-windows-x64-setup.exe";
  const fixturePath = path.join(fixtureDir, fixtureName);
  fs.writeFileSync(fixturePath, "fixture");
  try {
    const common = {
      repository: "Duojiyi/chimera-codex",
      version: "1.2.35-chimera.1",
      tag: "v1.2.35-chimera.1",
      assetDirectory: fixtureDir,
      writeFiles: false,
    };
    const defaultFloor = generateManifest({ ...common, minimumSupportedVersion: "" });
    assert.equal(defaultFloor.minimum_supported_version, "v1.2.35-chimera.1");
    assert.equal(defaultFloor.assets.length, 1);

    const crossUpstream = generateManifest({
      ...common,
      minimumSupportedVersion: "v1.2.34-chimera.3",
    });
    assert.equal(crossUpstream.minimum_supported_version, "v1.2.34-chimera.3");
    validateManifestFloor(crossUpstream, "1.2.34-chimera.3");

    assertThrows(
      () => generateManifest({ ...common, minimumSupportedVersion: "1.2.34-beta.1" }),
      /Invalid MINIMUM_SUPPORTED_VERSION/,
    );
    assertThrows(
      () => generateManifest({ ...common, minimumSupportedVersion: "not-a-version" }),
      /Invalid MINIMUM_SUPPORTED_VERSION/,
    );
    assertThrows(
      () => generateManifest({ ...common, minimumSupportedVersion: "1.2.35-chimera.2" }),
      /exceeds release/,
    );
    assertThrows(
      () =>
        generateManifest({
          ...common,
          minimumSupportedVersion: "0.18446744073709551616.0-chimera.1",
        }),
      /outside u64 range/,
    );
    assertThrows(
      () => validateManifestFloor({ version: "v1.2.35-chimera.1" }),
      /missing minimum_supported_version/,
    );
    assertThrows(
      () =>
        validateManifestFloor({
          version: "v1.2.35-chimera.1",
          minimum_supported_version: null,
        }),
      /minimum_supported_version must be a string/,
    );
    assertThrows(
      () =>
        validateManifestFloor({
          version: "v1.2.35-chimera.1",
          minimum_supported_version: "v1.2.36-chimera.1",
        }),
      /exceeds release/,
    );
  } finally {
    fs.unlinkSync(fixturePath);
    fs.rmdirSync(fixtureDir);
  }
  console.log("release-manifest self-test: PASS");
}

function main(args) {
  const [command, argument] = args;
  if (command === "--self-test") {
    selfTest();
    return;
  }
  if (command === "--validate-floor") {
    if (!argument) throw new Error("--validate-floor requires a manifest path");
    const payload = JSON.parse(fs.readFileSync(argument, "utf8"));
    validateManifestFloor(payload, process.env.MINIMUM_SUPPORTED_VERSION || "");
    console.log(`release-manifest floor validation: PASS (${argument})`);
    return;
  }
  if (command === "--generate") {
    generateManifest({
      repository: process.env.REPO,
      version: process.env.VERSION,
      tag: process.env.TAG,
      minimumSupportedVersion: process.env.MINIMUM_SUPPORTED_VERSION || "",
      assetDirectory: argument || "release-assets",
    });
    return;
  }
  throw new Error("usage: release-manifest.mjs --self-test | --validate-floor PATH | --generate [DIR]");
}

const isMain = process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (isMain) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
