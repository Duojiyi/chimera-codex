#!/usr/bin/env node
// Generate the first Chimera app-update trust root.
//
// This is an offline ceremony helper. It writes private role keys below
// CHIMERA_KEY_DIR (or ~/.chimera-keys/chimera-app-v1) and writes only public
// root metadata plus its detached signature to the requested output path.
// Never run this from CI and never commit the generated private-key directory.
import { generateKeyPairSync, sign } from "node:crypto";
import { mkdirSync, writeFileSync, chmodSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

const DOMAIN = "chimera-app-update.v1";
const ROOT_VERSION = 1;
const ROOT_EXPIRES = 2082758400; // 2036-01-01T00:00:00Z
const roles = ["root", "targets", "snapshot", "timestamp"];

const keyDir = process.env.CHIMERA_KEY_DIR || join(homedir(), ".chimera-keys", "chimera-app-v1");
const output = process.env.CHIMERA_ROOT_OUTPUT || join(keyDir, "root-metadata.json");

mkdirSync(keyDir, { recursive: true });
chmodSync(keyDir, 0o700);

const entries = [];
const privateKeys = new Map();
for (const role of roles) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const keyId = `chimera-app-${role}-v1`;
  const privatePem = privateKey.export({ type: "pkcs8", format: "pem" }).toString();
  const publicBytes = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  const privatePath = join(keyDir, `${role}.pkcs8.pem`);
  writeFileSync(privatePath, privatePem, { mode: 0o600 });
  chmodSync(privatePath, 0o600);
  entries.push({ key_id: keyId, public_key_hex: publicBytes.toString("hex") });
  privateKeys.set(role, privateKey);
}

const keyIds = Object.fromEntries(roles.map((role) => [role, `chimera-app-${role}-v1`]));
const root = {
  domain: DOMAIN,
  version: ROOT_VERSION,
  expires: ROOT_EXPIRES,
  keys: entries,
  root: { key_ids: [keyIds.root], threshold: 1 },
  targets: { key_ids: [keyIds.targets], threshold: 1 },
  snapshot: { key_ids: [keyIds.snapshot], threshold: 1 },
  timestamp: { key_ids: [keyIds.timestamp], threshold: 1 },
};

const payload = JSON.stringify(root);
const signatureHex = sign(null, Buffer.from(payload.trim(), "utf8"), privateKeys.get("root")).toString("hex");
const document = {
  payload,
  signatures: [{ key_id: keyIds.root, signature_hex: signatureHex }],
};

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, `${JSON.stringify(document, null, 2)}\n`, { mode: 0o600 });
chmodSync(output, 0o600);

console.log(`Generated app trust root v${ROOT_VERSION}`);
console.log(`Private role keys: ${keyDir}`);
console.log(`Public root document: ${output}`);
console.log(`Root key id: ${keyIds.root}`);
