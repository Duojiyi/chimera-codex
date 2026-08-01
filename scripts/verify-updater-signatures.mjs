#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const trustedCommentPrefix = "trusted comment: ";
const ed25519SpkiPrefix = Buffer.from("302a300506032b6570032100", "hex");

function usage() {
  console.error(
    "Usage: node scripts/verify-updater-signatures.mjs --assets-dir <dir> [--config <tauri.conf.json>] [--asset <artifact>]...",
  );
  process.exit(2);
}

const args = process.argv.slice(2);
function valueAfter(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}
function valuesAfter(flag) {
  const values = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === flag) {
      const value = args[index + 1];
      if (!value || value.startsWith("--")) usage();
      values.push(value);
      index += 1;
    }
  }
  return values;
}

const assetsDirArg = valueAfter("--assets-dir");
if (!assetsDirArg) usage();
const assetsDir = path.resolve(assetsDirArg);
const configPath = path.resolve(valueAfter("--config") ?? path.join(root, "src-tauri", "tauri.conf.json"));
const requestedAssets = valuesAfter("--asset");

function fail(message) {
  throw new Error(message);
}

function strictBase64(value, label) {
  const compact = value.replace(/\s+/g, "");
  if (!compact || compact.length % 4 !== 0 || !/^[A-Za-z0-9+/]+={0,2}$/.test(compact)) {
    fail(`${label} is not strict base64`);
  }
  const decoded = Buffer.from(compact, "base64");
  if (decoded.toString("base64") !== compact) {
    fail(`${label} has invalid base64 padding or characters`);
  }
  return decoded;
}

function trimTrailingEmptyLines(text) {
  const lines = text.replace(/^\uFEFF/, "").split(/\r?\n/);
  while (lines.length > 0 && lines.at(-1) === "") lines.pop();
  return lines;
}

function loadPublicKey(configFile) {
  const config = JSON.parse(fs.readFileSync(configFile, "utf8").replace(/^\uFEFF/, ""));
  const encoded = config?.plugins?.updater?.pubkey;
  if (typeof encoded !== "string" || encoded.length === 0) {
    fail(`Missing plugins.updater.pubkey in ${configFile}`);
  }
  const keyText = strictBase64(encoded, "tauri updater public key").toString("utf8");
  const lines = trimTrailingEmptyLines(keyText);
  if (lines.length !== 2 || !lines[0].startsWith("untrusted comment:")) {
    fail("Updater public key must be a two-line minisign public key");
  }
  const envelope = strictBase64(lines[1], "minisign public-key envelope");
  if (envelope.length !== 42) {
    fail(`Minisign public-key envelope must be 42 bytes, got ${envelope.length}`);
  }
  const algorithm = envelope.subarray(0, 2).toString("ascii");
  if (algorithm !== "Ed" && algorithm !== "ED") {
    fail(`Unsupported minisign public-key algorithm ${JSON.stringify(algorithm)}`);
  }
  const keyId = envelope.subarray(2, 10);
  const rawPublicKey = envelope.subarray(10, 42);
  const publicKey = crypto.createPublicKey({
    key: Buffer.concat([ed25519SpkiPrefix, rawPublicKey]),
    format: "der",
    type: "spki",
  });
  return { keyId, publicKey };
}

function decodeSignatureText(raw, label) {
  const normalized = raw.replace(/^\uFEFF/, "").trim();
  if (normalized.startsWith("untrusted comment:")) return normalized;

  const decoded = strictBase64(normalized, `${label} outer signature wrapper`).toString("utf8");
  if (!decoded.startsWith("untrusted comment:")) {
    fail(`${label} is neither a minisign signature nor a base64-wrapped minisign signature`);
  }
  return decoded;
}

function parseSignature(signatureText, label) {
  const lines = trimTrailingEmptyLines(signatureText);
  if (lines.length !== 4) {
    fail(`${label} must contain exactly four minisign signature lines`);
  }
  if (!lines[0].startsWith("untrusted comment:")) {
    fail(`${label} is missing an untrusted comment`);
  }
  if (!lines[2].startsWith(trustedCommentPrefix)) {
    fail(`${label} is missing a trusted comment`);
  }
  const envelope = strictBase64(lines[1], `${label} signature envelope`);
  const globalSignature = strictBase64(lines[3], `${label} global signature`);
  if (envelope.length !== 74) {
    fail(`${label} signature envelope must be 74 bytes, got ${envelope.length}`);
  }
  if (globalSignature.length !== 64) {
    fail(`${label} global signature must be 64 bytes, got ${globalSignature.length}`);
  }
  const algorithm = envelope.subarray(0, 2).toString("ascii");
  if (algorithm !== "ED") {
    fail(`${label} must use pre-hashed minisign algorithm ED, got ${JSON.stringify(algorithm)}`);
  }
  const trustedComment = lines[2].slice(trustedCommentPrefix.length);
  if (!trustedComment) {
    fail(`${label} trusted comment must not be empty`);
  }
  return {
    keyId: envelope.subarray(2, 10),
    signature: envelope.subarray(10, 74),
    trustedComment,
    globalSignature,
  };
}

async function blake2b512(file) {
  const hash = crypto.createHash("blake2b512");
  await new Promise((resolve, reject) => {
    const stream = fs.createReadStream(file);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", resolve);
    stream.on("error", reject);
  });
  return hash.digest();
}

function assertSafeArtifactName(name) {
  if (!name || path.basename(name) !== name || name === "." || name === ".." || name.includes("\\")) {
    fail(`Unsafe artifact name: ${JSON.stringify(name)}`);
  }
}

async function verifyArtifact({ assetName, assetsDir: dir, publicKey, keyId }) {
  assertSafeArtifactName(assetName);
  const artifact = path.join(dir, assetName);
  const signatureFile = `${artifact}.sig`;
  if (!fs.statSync(artifact, { throwIfNoEntry: false })?.isFile()) {
    fail(`Updater artifact is missing: ${artifact}`);
  }
  if (!fs.statSync(signatureFile, { throwIfNoEntry: false })?.isFile()) {
    fail(`Updater signature is missing: ${signatureFile}`);
  }

  const parsed = parseSignature(
    decodeSignatureText(fs.readFileSync(signatureFile, "utf8"), path.basename(signatureFile)),
    path.basename(signatureFile),
  );
  if (!crypto.timingSafeEqual(parsed.keyId, keyId)) {
    fail(`Updater signature key id does not match configured updater public key: ${assetName}`);
  }

  const digest = await blake2b512(artifact);
  if (!crypto.verify(null, digest, publicKey, parsed.signature)) {
    fail(`Pre-hashed minisign signature verification failed: ${assetName}`);
  }
  const globalMessage = Buffer.concat([parsed.signature, Buffer.from(parsed.trustedComment, "utf8")]);
  if (!crypto.verify(null, globalMessage, publicKey, parsed.globalSignature)) {
    fail(`Minisign trusted-comment signature verification failed: ${assetName}`);
  }
  console.log(`Verified updater signature: ${assetName}`);
}

if (!fs.statSync(assetsDir, { throwIfNoEntry: false })?.isDirectory()) {
  fail(`Assets directory does not exist: ${assetsDir}`);
}

const assets = requestedAssets.length
  ? requestedAssets
  : fs
      .readdirSync(assetsDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".sig"))
      .map((entry) => entry.name.slice(0, -4))
      .sort();
if (assets.length === 0) {
  fail(`No updater signature files found in ${assetsDir}`);
}
if (new Set(assets).size !== assets.length) {
  fail("Updater artifact list contains duplicates");
}

const { keyId, publicKey } = loadPublicKey(configPath);
for (const assetName of assets) {
  await verifyArtifact({ assetName, assetsDir, publicKey, keyId });
}
console.log(`Verified ${assets.length} updater signature(s) with the configured updater public key.`);
