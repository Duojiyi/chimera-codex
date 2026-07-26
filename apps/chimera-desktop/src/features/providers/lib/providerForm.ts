// Chimera++ 2.0 — Provider form validation logic (pure TS, no React, no Tauri).
// G12: 此模块只做数据验证，不读写文件，不调用 Tauri invoke。

export interface ProviderFormInput {
  url: string;
  apiKey: string;
}

export interface ProviderFormError {
  field: "url" | "apiKey" | "general";
  message: string;      // user-facing, no raw HTTP/Rust errors
  recovery: string;     // actionable next step
  severity: "error" | "warning";
}

// ── ChimeraHub Key-first ─────────────────────────────────────────────────────

/**
 * Validate a ChimeraHub API key (first-run flow).
 * Returns null if valid, or a ProviderFormError if invalid.
 */
export function validateChimeraHubKey(
  rawKey: string,
): ProviderFormError | null {
  const key = rawKey.trim();
  if (key.length === 0) {
    return {
      field: "apiKey",
      message: "API Key is required to connect to ChimeraHub.",
      recovery: "Enter your ChimeraHub API Key. You can find it at api.chimerahub.org/dashboard.",
      severity: "error",
    };
  }
  return null;
}

// ── Custom URL + Key ─────────────────────────────────────────────────────────

const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1"]);

function isLoopback(url: URL): boolean {
  return LOOPBACK_HOSTS.has(url.hostname);
}

/**
 * Validate custom provider input (two-field: URL + Key).
 * Returns an array of ProviderFormError (empty = valid).
 * Warnings (severity="warning") are non-blocking but require user confirmation.
 */
export function validateCustomProviderInput(
  input: ProviderFormInput,
): ProviderFormError[] {
  const errors: ProviderFormError[] = [];

  // ── URL ──────────────────────────────────────────────────────────────────
  const rawUrl = input.url.trim();
  if (rawUrl.length === 0) {
    errors.push({
      field: "url",
      message: "Provider URL is required.",
      recovery: "Enter the base URL of your provider's API endpoint (e.g. https://api.example.com/v1).",
      severity: "error",
    });
  } else {
    let parsed: URL;
    try {
      parsed = new URL(rawUrl);
    } catch {
      errors.push({
        field: "url",
        message: "The URL you entered could not be parsed.",
        recovery: "Make sure the URL starts with https:// and is a valid web address.",
        severity: "error",
      });
      // Can't do further URL checks without a parsed URL
      return finishWithKeyCheck(errors, input.apiKey);
    }

    // Scheme check
    if (parsed.protocol === "http:") {
      if (!isLoopback(parsed)) {
        errors.push({
          field: "url",
          message: "Plain HTTP is not allowed for remote providers. Only HTTPS ensures your API Key is transmitted securely.",
          recovery: "Replace http:// with https:// in your provider URL.",
          severity: "error",
        });
      }
      // If loopback + http, allow but warn (production dev_mode handling done in Rust)
    } else if (parsed.protocol !== "https:") {
      errors.push({
        field: "url",
        message: `The URL scheme "${parsed.protocol.replace(":", "")}" is not supported.`,
        recovery: "Use an https:// URL for your provider endpoint.",
        severity: "error",
      });
    }

    // Userinfo ban
    if (parsed.username || parsed.password) {
      errors.push({
        field: "url",
        message: "The URL contains embedded credentials (user:pass@host). This is not allowed.",
        recovery: "Remove the username and password from the URL. Enter your API Key in the Key field below.",
        severity: "error",
      });
    }

    // Origin-only warning (no path beyond /)
    if (
      parsed.pathname === "/" ||
      parsed.pathname === ""
    ) {
      errors.push({
        field: "url",
        message: "The URL looks like an origin only (no path). Chimera will try /v1 as the endpoint.",
        recovery: "If your provider uses a different path (e.g. /api/v1), enter the full URL with the path.",
        severity: "warning",
      });
    }
  }

  return finishWithKeyCheck(errors, input.apiKey);
}

function finishWithKeyCheck(
  errors: ProviderFormError[],
  rawKey: string,
): ProviderFormError[] {
  const key = rawKey.trim();
  if (key.length === 0) {
    errors.push({
      field: "apiKey",
      message: "API Key is required.",
      recovery: "Enter the API Key for this provider. Keys are stored securely in your OS keychain.",
      severity: "error",
    });
  }
  return errors;
}
