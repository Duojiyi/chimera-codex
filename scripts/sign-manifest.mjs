#!/usr/bin/env node
// Sign a release manifest with the Chimera app-domain ed25519 key.
//
// ADR-008 dropped OS code signing; this is what replaced it. It is not a
// substitute for Authenticode — it does nothing about SmartScreen — but it is
// what decides whether a client will install an update, which is the property
// that actually protects users.
//
// Trust-domain rule (G15, ADR-006): this key signs CHIMERA APP releases only.
// The Codex mirror has its own, separately-rotated key. They must never be the
// same key, and this script refuses to run if it is handed the mirror's.
//
// Uses Node's built-in ed25519 support rather than a library, so the release
// path has no dependency that could be substituted underneath it.
//
// Usage:
//   node scripts/sign-manifest.mjs --in <manifest.json> --out <sig-file>
//   node scripts/sign-manifest.mjs --self-test
//   node scripts/sign-manifest.mjs --generate-key      (prints a new keypair)
//
// The private key is read from CHIMERA_APP_SIGNING_KEY (base64 PKCS#8) and is
// never written to disk, echoed, or included in an error message.
import { createPrivateKey, createPublicKey, sign, verify, generateKeyPairSync } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const KEY_ENV = "CHIMERA_APP_SIGNING_KEY";

/** Marks a key as belonging to the mirror domain, which must not sign app releases. */
const MIRROR_KEY_ENV = "CHIMERA_MIRROR_SIGNING_KEY";

/**
 * Canonical bytes to sign.
 *
 * The signature covers a re-serialised, key-sorted form rather than the file's
 * raw bytes, so a verifier that reformats the JSON — or a mirror that
 * re-emits it with different whitespace — still validates. Signing raw bytes
 * would make the signature depend on formatting nobody controls end to end.
 */
export function canonicalBytes(manifest) {
  const sortDeep = (v) =>
    Array.isArray(v)
      ? v.map(sortDeep)
      : v && typeof v === "object"
        ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortDeep(v[k])]))
        : v;
  return Buffer.from(JSON.stringify(sortDeep(manifest)), "utf8");
}

export function signManifest(manifest, privateKeyPem) {
  const key = createPrivateKey(privateKeyPem);
  if (key.asymmetricKeyType !== "ed25519") {
    throw new Error(`signing key must be ed25519, got ${key.asymmetricKeyType}`);
  }
  return sign(null, canonicalBytes(manifest), key).toString("base64");
}

export function verifyManifest(manifest, signatureB64, publicKeyPem) {
  const key = createPublicKey(publicKeyPem);
  if (key.asymmetricKeyType !== "ed25519") return false;
  try {
    return verify(null, canonicalBytes(manifest), key, Buffer.from(signatureB64, "base64"));
  } catch {
    // A malformed signature is a failed verification, not a crash. Fail closed.
    return false;
  }
}

function loadSigningKey() {
  const raw = process.env[KEY_ENV];
  if (!raw) throw new Error(`${KEY_ENV} is not set`);
  if (process.env[MIRROR_KEY_ENV] && process.env[MIRROR_KEY_ENV] === raw) {
    throw new Error(
      `${KEY_ENV} is the same key as ${MIRROR_KEY_ENV}. The app and mirror trust ` +
        `domains must be independently rotatable and revocable (G15, ADR-006).`,
    );
  }
  // Accept either raw PEM or base64-wrapped PEM, since CI secret editors
  // mangle multi-line values in different ways.
  const pem = raw.includes("BEGIN") ? raw : Buffer.from(raw, "base64").toString("utf8");
  if (!pem.includes("BEGIN PRIVATE KEY")) {
    // Deliberately does not echo any part of the value.
    throw new Error(`${KEY_ENV} is not a PKCS#8 PEM private key`);
  }
  return pem;
}

// ── Self-test ──────────────────────────────────────────────────────────────

function selfTest() {
  let failures = 0;
  const check = (label, ok, detail = "") => {
    console.log(`${ok ? "\x1b[32m✓\x1b[0m" : "\x1b[31m✗\x1b[0m"} ${label}${detail ? `: ${detail}` : ""}`);
    if (!ok) failures++;
  };

  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const priv = privateKey.export({ type: "pkcs8", format: "pem" });
  const pub = publicKey.export({ type: "spki", format: "pem" });

  const manifest = { schema_version: 1, version: "2.0.0", assets: [{ name: "a.exe", sha256: "ab".repeat(32) }] };
  const sig = signManifest(manifest, priv);

  check("a signature over an unmodified manifest verifies", verifyManifest(manifest, sig, pub));

  // Same content, different key order and whitespace — which is exactly what a
  // mirror that re-emits the JSON produces. Canonicalisation is what makes
  // that survive; signing raw file bytes would not.
  const reordered = {};
  for (const k of Object.keys(manifest).reverse()) reordered[k] = manifest[k];
  check(
    "reordering keys does not break the signature",
    verifyManifest(JSON.parse(JSON.stringify(reordered, null, 4)), sig, pub),
  );

  check(
    "changing an asset digest invalidates the signature",
    !verifyManifest(
      { ...manifest, assets: [{ name: "a.exe", sha256: "cd".repeat(32) }] },
      sig,
      pub,
    ),
  );

  check(
    "changing the version invalidates the signature",
    !verifyManifest({ ...manifest, version: "2.0.1" }, sig, pub),
  );

  check(
    "adding an asset invalidates the signature",
    !verifyManifest(
      { ...manifest, assets: [...manifest.assets, { name: "evil.exe", sha256: "ff".repeat(32) }] },
      sig,
      pub,
    ),
  );

  const other = generateKeyPairSync("ed25519");
  check(
    "a different key does not verify",
    !verifyManifest(manifest, sig, other.publicKey.export({ type: "spki", format: "pem" })),
  );

  check("a garbage signature is rejected rather than throwing", !verifyManifest(manifest, "not-base64!!", pub));
  check("an empty signature is rejected", !verifyManifest(manifest, "", pub));

  // G15: the two trust domains must not share a key.
  const before = { app: process.env[KEY_ENV], mirror: process.env[MIRROR_KEY_ENV] };
  process.env[KEY_ENV] = Buffer.from(priv).toString("base64");
  process.env[MIRROR_KEY_ENV] = process.env[KEY_ENV];
  let refused = false;
  try { loadSigningKey(); } catch (e) { refused = /independently rotatable/.test(e.message); }
  check("refuses to sign when the app key is also the mirror key", refused);
  if (before.app === undefined) delete process.env[KEY_ENV]; else process.env[KEY_ENV] = before.app;
  if (before.mirror === undefined) delete process.env[MIRROR_KEY_ENV]; else process.env[MIRROR_KEY_ENV] = before.mirror;

  // An RSA key must be refused: ed25519 is the algorithm the client verifies.
  const rsa = generateKeyPairSync("rsa", { modulusLength: 2048 });
  let rsaRefused = false;
  try { signManifest(manifest, rsa.privateKey.export({ type: "pkcs8", format: "pem" })); }
  catch (e) { rsaRefused = /must be ed25519/.test(e.message); }
  check("refuses a non-ed25519 signing key", rsaRefused);

  console.log(failures === 0 ? "\nsign-manifest self-test: PASS" : `\nsign-manifest self-test: ${failures} failure(s)`);
  process.exit(failures > 0 ? 1 : 0);
}

// ── CLI ────────────────────────────────────────────────────────────────────

const argv = process.argv.slice(2);
const flag = (n) => { const i = argv.indexOf(n); return i === -1 ? null : argv[i + 1]; };

if (argv.includes("--self-test")) {
  selfTest();
} else if (argv.includes("--generate-key")) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  console.error("Store the PRIVATE key as a protected environment secret. It is printed once.");
  console.log("### PUBLIC KEY (embed in the client) ###");
  console.log(publicKey.export({ type: "spki", format: "pem" }).toString().trim());
  console.log(`### ${KEY_ENV} (base64 PKCS#8) ###`);
  console.log(Buffer.from(privateKey.export({ type: "pkcs8", format: "pem" })).toString("base64"));
} else {
  const inPath = flag("--in");
  const outPath = flag("--out");
  if (!inPath || !outPath) {
    console.error("usage: sign-manifest.mjs --in <manifest.json> --out <sig> | --self-test | --generate-key");
    process.exit(2);
  }
  try {
    const manifest = JSON.parse(readFileSync(inPath, "utf8"));
    const signature = signManifest(manifest, loadSigningKey());
    writeFileSync(outPath, `${signature}\n`);
    console.log(`signed ${inPath} -> ${outPath}`);
  } catch (e) {
    // Never print the exception's own toString: a crypto error can carry key
    // bytes in its message on some Node builds.
    console.error(`signing failed: ${e instanceof Error ? e.message : "unknown error"}`);
    process.exit(1);
  }
}
