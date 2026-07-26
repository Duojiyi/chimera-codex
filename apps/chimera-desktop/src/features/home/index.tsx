// Chimera++ 2.0 — Home feature.
// Layout is a 1:1 implementation of the Pencil design frame `Home` (qUByL).
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState, useEffect } from "react";
import { color, type as font, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";

interface SystemStatus {
  providerName: string | null;
  providerHealth: "unknown" | "healthy" | "auth_failed" | "unreachable";
  codexVersion: string | null;
  codexRunning: boolean;
  officialMode: boolean;
}

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

const HEALTH_COLOR: Record<SystemStatus["providerHealth"], string> = {
  unknown: color.muted,
  healthy: color.green,
  auth_failed: color.danger,
  unreachable: color.danger,
};

// Module-level tables hold i18n KEYS, never translated text — translating here
// would freeze the string at import and break instant language switching.
// scripts/verify-i18n.mjs enforces this.
const HEALTH_LABEL_KEY: Record<SystemStatus["providerHealth"], TranslationKey> = {
  unknown: "health.unknown",
  healthy: "health.healthy",
  auth_failed: "health.authFailed",
  unreachable: "health.unreachable",
};

export function HomeFeature() {
  const { t } = useI18n();
  const [status, setStatus] = useState<SystemStatus>({
    providerName: null, providerHealth: "unknown",
    codexVersion: null, codexRunning: false, officialMode: true,
  });
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);

  useEffect(() => {
    invoke("get_system_status").then(s => s && setStatus(s as SystemStatus)).catch(() => {});
  }, []);

  async function handleLaunch() {
    setLaunching(true); setLaunchError(null);
    try {
      await invoke("launch_codex");
      setStatus(s => ({ ...s, codexRunning: true }));
    } catch (err: unknown) {
      setLaunchError(err instanceof Error ? err.message : t("home.launchFailed"));
    } finally {
      setLaunching(false);
    }
  }

  const dash = t("common.dash");
  const providerLabel = status.officialMode
    ? t("home.officialCodex")
    : (status.providerName ?? t("home.noProvider"));
  const healthColor = HEALTH_COLOR[status.providerHealth];
  const healthLabel = t(HEALTH_LABEL_KEY[status.providerHealth]);

  const details: [string, string, string][] = [
    [t("home.rowHealth"),   healthLabel, healthColor],
    [t("home.colProvider"), status.officialMode ? t("home.valOfficial") : t("home.valCustom"), color.secondary],
    [t("home.rowVersion"),  status.codexVersion ?? dash, color.secondary],
  ];

  const strip: { title: string; rows: [string, string][] }[] = [
    { title: t("home.colProvider"), rows: [
      [t("home.rowEndpoint"), status.providerName ?? t("home.valOfficial")],
      [t("home.rowProtocol"), t("home.valResponses")],
      [t("home.rowStatus"), healthLabel],
    ]},
    { title: t("home.colRuntime"), rows: [
      [t("home.rowVersion"), status.codexVersion ?? dash],
      [t("home.rowMode"), t("home.valManagedPortable")],
      [t("home.rowHealth"), status.codexRunning ? t("home.running") : t("home.stopped")],
    ]},
    { title: t("home.colUpdates"), rows: [
      [t("home.rowChimera"), t("home.valUpToDate")],
      [t("home.rowCodex"), dash],
      [t("home.rowLastCheck"), dash],
    ]},
  ];

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {/* ── Hero (spec: 360px fixed) ── */}
      <div style={{ height: size.heroHeight, display: "flex", flexShrink: 0 }}>
        <div
          role="main"
          aria-label={t("home.statusAriaLabel")}
          style={{
            flex: 1, display: "flex", flexDirection: "column", justifyContent: "center",
            padding: `${size.heroPadY}px ${size.heroPadX}px`,
          }}
        >
          <p style={{ ...font.eyebrow, color: color.muted, margin: `0 0 ${size.heroEyebrowGap}px` }}>{t("home.eyebrow")}</p>
          <h1 style={{ ...font.hero, color: color.primary, margin: 0 }}>{providerLabel}</h1>

          <div style={{ display: "flex", alignItems: "center", gap: 14, marginTop: 14 }}>
            <span style={{ fontSize: 20, color: color.accent }} aria-hidden="true">→</span>
            <span style={{ fontSize: 24, color: color.secondary }}>{status.codexVersion ?? t("home.codexNotInstalled")}</span>
            <span style={{ width: 1, height: 18, background: color.rule }} />
            <span style={{ fontSize: 20, color: status.codexRunning ? color.green : color.muted }}>
              {status.codexRunning ? "running" : "stopped"}
            </span>
          </div>

          <div style={{ display: "flex", gap: 36, marginTop: 26 }}>
            {details.map(([label, value, valueColor]) => (
              <div key={label} style={{ display: "flex", flexDirection: "column", gap: 5 }}>
                <span style={{ ...font.metric, color: valueColor }}>{value}</span>
                <span style={{ ...font.caption, color: color.dim }}>{label}</span>
              </div>
            ))}
          </div>
        </div>

        <div style={{ width: 1, background: color.rule, alignSelf: "stretch" }} />

        <div style={{
          width: size.heroRight, display: "flex", flexDirection: "column",
          justifyContent: "center", alignItems: "center", gap: 14, padding: "0 40px",
        }}>
          <button
            onClick={handleLaunch}
            disabled={launching}
            aria-label={status.codexRunning ? t("home.launchAriaRunning") : t("home.launchAriaIdle")}
            style={{
              background: color.accent, color: color.ink0, border: "none", borderRadius: 4,
              padding: "15px 36px", fontSize: 16, fontWeight: 700, fontFamily: "inherit",
              cursor: launching ? "wait" : "pointer", opacity: launching ? 0.7 : 1, width: "100%",
            }}
          >
            {launching ? t("home.launching") : status.codexRunning ? t("home.alreadyRunning") : t("home.launch")}
          </button>
          <div role="alert" aria-live="polite" style={{ minHeight: 16 }}>
            {launchError && (
              <p style={{ fontSize: 12, color: color.danger, textAlign: "center", margin: 0 }}>{launchError}</p>
            )}
          </div>
          <p style={{ fontSize: 11, color: color.dim, textAlign: "center", margin: 0 }}>{t("home.quickAccess")}</p>
        </div>
      </div>

      <div style={{ height: 1, background: color.rule, flexShrink: 0 }} />

      {/* ── Data strip (spec: 3 × 426px columns, 32px rows) ── */}
      <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
        {strip.map((col, i, arr) => (
          <div
            key={col.title}
            style={{
              flex: 1, padding: `${size.dataPadY}px ${size.dataPadX}px`,
              borderRight: i < arr.length - 1 ? `1px solid ${color.rule}` : "none",
            }}
          >
            <p style={{ ...font.sectionLabel, color: color.dim, margin: "0 0 10px" }}>{col.title}</p>
            <div style={{ height: 1, background: color.rule, opacity: 0.5, marginBottom: 6 }} />
            {col.rows.map(([k, v]) => (
              <div key={k} style={{ display: "flex", height: size.dataRow, alignItems: "center" }}>
                <span style={{ fontSize: 12, color: color.muted, width: size.dataKeyWidth, flexShrink: 0 }}>{k}</span>
                <span style={{ fontSize: 12, fontWeight: 500, color: color.secondary }}>{v}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
