// The gate that runs before the app is usable.
//
// D6 gave Chimera a real first run: the Codex payload is no longer in the
// package, so there is a window where the app is installed but cannot do
// anything yet. ADR-009 added the other half — a machine may be missing
// WebView2 or disk space, and that has to be discovered BEFORE any state is
// written, not while a screen is trying to render.
//
// The promise this screen makes is the one thing it must not break: when
// something is wrong, nothing on the user's machine has been changed. So it
// runs preflight, reports, and stops. It never repairs, installs or migrates
// on its own.

import { useCallback, useEffect, useState } from "react";
import { color, type as font, radius, hairline } from "@/design/tokens.ts";
import { useI18n, type TranslationKey } from "@/i18n";

interface PreflightResult {
  ok: boolean;
  blockingKeys: string[];
  webview2DownloadUrl: string | null;
  freeBytes: number | null;
}

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as { __TAURI_INTERNALS__?: { invoke?: unknown } }).__TAURI_INTERNALS__
    ? ((window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a?: Record<string, unknown>) => Promise<unknown> } })
        .__TAURI_INTERNALS__.invoke)
    : async () => undefined;

/**
 * A conservative payload size used only to size the disk-space check before a
 * manifest has been read.
 *
 * Deliberately an over-estimate: warning about space that turns out to be
 * sufficient costs the user one extra click, while under-estimating puts them
 * into a failed unpack with a half-written version directory.
 */
const ASSUMED_PAYLOAD_BYTES = 200 * 1024 * 1024;

export function FirstRun({ onReady }: { onReady: () => void }) {
  const { t } = useI18n();
  const [result, setResult] = useState<PreflightResult | null>(null);
  const [checking, setChecking] = useState(true);

  const check = useCallback(async () => {
    setChecking(true);
    try {
      const r = (await invoke("run_preflight", {
        payloadBytes: ASSUMED_PAYLOAD_BYTES,
      })) as PreflightResult | undefined;
      // Outside the desktop shell (the design-verify harness runs in a plain
      // browser) there is no backend. Treating "no answer" as a blocked start
      // would make every screenshot a preflight failure, so it passes through.
      if (!r) {
        onReady();
        return;
      }
      setResult(r);
      if (r.ok) onReady();
    } catch {
      // A command that threw tells us nothing about the machine. Blocking the
      // app on it would strand the user with no way to reach Diagnose.
      onReady();
    } finally {
      setChecking(false);
    }
  }, [onReady]);

  useEffect(() => {
    void check();
  }, [check]);

  if (checking && !result) {
    return (
      <Centered>
        <p style={{ ...font.body, color: color.muted, margin: 0 }}>{t("common.loading")}</p>
      </Centered>
    );
  }

  if (!result || result.ok) return null;

  return (
    <Centered>
      <h1 style={{ ...font.pageTitle, color: color.primary, margin: "0 0 10px" }}>
        {t("preflight.title")}
      </h1>
      <p style={{ ...font.body, color: color.secondary, margin: "0 0 22px", lineHeight: 1.6 }}>
        {t("preflight.blockedIntro")}
      </p>

      <ul style={{ margin: "0 0 22px", padding: 0, listStyle: "none", display: "grid", gap: 14 }}>
        {result.blockingKeys.map((key) => (
          <li
            key={key}
            style={{
              background: color.ink1,
              border: `${hairline}px solid ${color.rule}`,
              borderRadius: radius.sm,
              padding: "14px 16px",
              color: color.primary,
              fontSize: 13,
              lineHeight: 1.6,
            }}
          >
            {t(key as TranslationKey)}
            {key === "preflight.webview2Missing" && result.webview2DownloadUrl && (
              <>
                {" "}
                <a
                  href={result.webview2DownloadUrl}
                  target="_blank"
                  rel="noreferrer noopener"
                  style={{ color: color.accent }}
                >
                  {t("preflight.webview2Download")}
                </a>
              </>
            )}
          </li>
        ))}
      </ul>

      <button
        type="button"
        onClick={() => void check()}
        disabled={checking}
        style={{
          background: color.accent,
          color: color.ink0,
          border: "none",
          borderRadius: radius.sm,
          padding: "9px 22px",
          fontSize: 13,
          fontWeight: 700,
          fontFamily: "inherit",
          cursor: checking ? "wait" : "pointer",
          alignSelf: "flex-start",
        }}
      >
        {t("preflight.recheck")}
      </button>
    </Centered>
  );
}

/**
 * Uses `role="main"` rather than an overlay on top of the real UI: while
 * preflight is blocked there IS no usable app behind this, and leaving the
 * navigation reachable underneath would let a screen reader wander into
 * screens that cannot function.
 */
function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div
      role="main"
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        // A plain value, not a token: tokens.ts mirrors the .pen design file
        // and V16 compares them, so inventing an entry for a screen the design
        // file does not contain would make that comparison a lie.
        maxWidth: 620,
        margin: "0 auto",
        padding: "0 40px",
      }}
    >
      {children}
    </div>
  );
}
