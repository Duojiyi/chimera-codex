#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const expectedIndex = args.indexOf("--expect");
const expected = expectedIndex === -1 ? undefined : args[expectedIndex + 1];
if (expectedIndex !== -1 && !expected) {
  throw new Error("--expect requires a version");
}

const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const packageVersion = packageJson.version;
if (typeof packageVersion !== "string" || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(packageVersion)) {
  throw new Error(`package.json contains an invalid version: ${JSON.stringify(packageVersion)}`);
}

const cargo = fs.readFileSync(path.join(root, "src-tauri", "Cargo.toml"), "utf8");
const packageSection = cargo.match(/^\[package\]\s*$([\s\S]*?)(?=^\[|\Z)/m)?.[1];
const cargoVersion = packageSection?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
if (!cargoVersion) {
  throw new Error("src-tauri/Cargo.toml [package] has no version");
}

const tauri = JSON.parse(fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const tauriVersion = tauri.version;

const lock = fs.readFileSync(path.join(root, "src-tauri", "Cargo.lock"), "utf8");
const lockVersion = lock.match(
  /^\[\[package\]\]\s*$\nname\s*=\s*"chimera-plus-plus"\s*$\nversion\s*=\s*"([^"]+)"\s*$/m,
)?.[1];
if (!lockVersion) {
  throw new Error("src-tauri/Cargo.lock has no chimera-plus-plus package version");
}

const versions = {
  "package.json": packageVersion,
  "src-tauri/Cargo.toml": cargoVersion,
  "src-tauri/tauri.conf.json": tauriVersion,
  "src-tauri/Cargo.lock": lockVersion,
};
if (new Set(Object.values(versions)).size !== 1) {
  throw new Error(`version mismatch: ${Object.entries(versions).map(([file, version]) => `${file}=${JSON.stringify(version)}`).join(", ")}`);
}
if (expected && packageVersion !== expected.replace(/^v/, "")) {
  throw new Error(`expected ${JSON.stringify(expected)}, found ${JSON.stringify(packageVersion)}`);
}
console.log(`Version consistency verified: ${packageVersion}`);
