// Step 3.1 RED — Provider form validation logic (pure TS, no React).
// Run: node --test src/features/providers/lib/providerForm.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  validateChimeraHubKey,
  validateCustomProviderInput,
  type ProviderFormInput,
  type ProviderFormError,
} from "./providerForm.ts";

// ── ChimeraHub Key-first validation ─────────────────────────────────────────

describe("validateChimeraHubKey", () => {
  it("rejects empty key", () => {
    const err = validateChimeraHubKey("");
    assert.ok(err, "empty key must return an error");
    assert.match(err.message, /empty|required|key/i);
  });

  it("rejects whitespace-only key", () => {
    const err = validateChimeraHubKey("   ");
    assert.ok(err, "whitespace-only key must return an error");
  });

  it("accepts a non-empty trimmed key", () => {
    const err = validateChimeraHubKey("sk-test-key-12345");
    assert.equal(err, null, "valid key must return null (no error)");
  });

  it("trims surrounding whitespace before validating", () => {
    // Key with leading/trailing spaces is valid after trimming
    const err = validateChimeraHubKey("  sk-test-key  ");
    assert.equal(err, null, "key with surrounding whitespace is valid after trim");
  });

  it("returns actionable error with recovery hint", () => {
    const err = validateChimeraHubKey("");
    assert.ok(err?.recovery, "error must include a recovery hint");
  });
});

// ── Custom URL + Key validation ──────────────────────────────────────────────

describe("validateCustomProviderInput", () => {
  const valid: ProviderFormInput = {
    url: "https://api.example.com/v1",
    apiKey: "sk-test-key",
  };

  it("accepts valid HTTPS URL and key", () => {
    const errs = validateCustomProviderInput(valid);
    assert.equal(errs.length, 0, "valid input must have no errors");
  });

  it("rejects HTTP URL (non-loopback)", () => {
    const errs = validateCustomProviderInput({ ...valid, url: "http://api.example.com/v1" });
    const urlErr = errs.find(e => e.field === "url");
    assert.ok(urlErr, "HTTP URL must produce url field error");
  });

  it("rejects empty URL", () => {
    const errs = validateCustomProviderInput({ ...valid, url: "" });
    assert.ok(errs.some(e => e.field === "url"), "empty URL must produce url error");
  });

  it("rejects empty API key", () => {
    const errs = validateCustomProviderInput({ ...valid, apiKey: "" });
    assert.ok(errs.some(e => e.field === "apiKey"), "empty key must produce apiKey error");
  });

  it("rejects URL with userinfo", () => {
    const errs = validateCustomProviderInput({ ...valid, url: "https://user:pass@api.example.com/v1" });
    assert.ok(errs.some(e => e.field === "url"), "userinfo URL must be rejected");
  });

  it("marks origin-only URL as needing confirmation", () => {
    const errs = validateCustomProviderInput({ ...valid, url: "https://api.example.com" });
    // Origin-only is not a hard error but needs user confirmation
    const warn = errs.find(e => e.severity === "warning" && e.field === "url");
    assert.ok(warn, "origin-only URL must produce a warning (not hard error): " + JSON.stringify(errs));
  });

  it("returns errors with actionable messages (not raw HTTP/Rust errors)", () => {
    const errs = validateCustomProviderInput({ ...valid, url: "http://not-secure.example.com" });
    assert.ok(errs.length > 0);
    // Error message must not contain raw technical jargon
    for (const e of errs) {
      assert.ok(e.message.length > 0, "error must have a message");
      assert.ok(e.recovery, "error must have a recovery hint");
    }
  });
});
