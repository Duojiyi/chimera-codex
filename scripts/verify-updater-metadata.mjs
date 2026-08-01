#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

function usage() {
  console.error("Usage: node scripts/verify-updater-metadata.mjs --file latest.json --tag vX.Y.Z --assets-dir release-assets");
  process.exit(2);
}

const args = process.argv.slice(2);
const valueAfter = (flag) => {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
};
const file = valueAfter("--file");
const tag = valueAfter("--tag");
const assetsDir = valueAfter("--assets-dir");
if (!file || !tag || !assetsDir || !tag.startsWith("v")) usage();

const expectedVersion = tag.slice(1);
const metadata = JSON.parse(fs.readFileSync(file, "utf8"));
if (metadata.version !== expectedVersion) {
  throw new Error(`latest.json version ${JSON.stringify(metadata.version)} does not match ${JSON.stringify(expectedVersion)}`);
}
if (!metadata.url?.endsWith(`/releases/tag/${tag}`)) {
  throw new Error("latest.json release URL does not point to this tag");
}
if (!metadata.platforms || typeof metadata.platforms !== "object") {
  throw new Error("latest.json has no updater platforms");
}
for (const [platform, entry] of Object.entries(metadata.platforms)) {
  if (!entry?.url?.includes(`/releases/download/${tag}/`) || !entry?.signature) {
    throw new Error(`invalid updater entry for ${platform}`);
  }
}
if (!Array.isArray(metadata.assets) || metadata.assets.length === 0) {
  throw new Error("latest.json has no release assets");
}
for (const asset of metadata.assets) {
  const name = path.basename(asset?.name || "");
  const assetPath = path.join(assetsDir, name);
  if (!name || !fs.existsSync(assetPath)) {
    throw new Error(`metadata asset is missing from ${assetsDir}: ${name || "<empty>"}`);
  }
  const actualSha = crypto.createHash("sha256").update(fs.readFileSync(assetPath)).digest("hex");
  if (asset.sha256 !== actualSha) {
    throw new Error(`SHA-256 mismatch for ${name}`);
  }
  if (asset.size !== fs.statSync(assetPath).size) {
    throw new Error(`size mismatch for ${name}`);
  }
}
console.log(`Updater metadata verified for ${tag}`);
