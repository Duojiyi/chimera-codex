// Feature: codex — Managed Codex runtime status, updates, and diagnostics.
// Layout is a 1:1 implementation of the Pencil design frame `Codex` (Body).
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState, useEffect } from "react";
import { color, type as font, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";

interface VersionEntry {
  version: string;
  state: string;
}

interface DiagnosticEntry {
  name: string;
  result: "pass" | "warn" | "fail";
}

interface RuntimeStatus {
  installed: boolean;
  version: string | null;
  platform: string | null;
  healthy: boolean;
  healthLabel: string | null;
  mode: string | null;
  ownership: string | null;
  installPath: string | null;
  lastUpdate: string | null;
  uptime: string | null;
  updateAvailable: boolean;
  updateVersion: string | null;
  updateChannel: string | null;
  updateMeta: string | null;
  history: VersionEntry[];
  diagnostics: DiagnosticEntry[];
}

const EMPTY_STATUS: RuntimeStatus = {
  installed: false,
  version: null,
  platform: null,
  healthy: false,
  healthLabel: null,
  mode: null,
  ownership: null,
  installPath: null,
  lastUpdate: null,
  uptime: null,
  updateAvailable: false,
  updateVersion: null,
  updateChannel: null,
  updateMeta: null,
  history: [],
  diagnostics: [],
};

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

const RESULT_COLOR: Record<DiagnosticEntry["result"], string> = {
  pass: color.green,
  warn: color.amber,
  fail: color.danger,
};

function SectionLabel({ children, textColor }: { children: string; textColor: string }) {
  return (
    <p style={{ ...font.sectionLabel, color: textColor, margin: 0 }}>{children}</p>
  );
}

function HairlineRule({ opacity = 1 }: { opacity?: number }) {
  return <div style={{ height: hairline, background: color.rule, opacity, flexShrink: 0 }} />;
}

export function CodexFeature() {
  const [status, setStatus] = useState<RuntimeStatus>(EMPTY_STATUS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke("get_runtime_status")
      .then(s => s && setStatus(s as RuntimeStatus))
      .catch(() => {});
  }, []);

  async function runAction(action: () => Promise<unknown>, failMessage: string) {
    setBusy(true);
    setError(null);
    try {
      await action();
      const s = await invoke("get_runtime_status").catch(() => undefined);
      if (s) setStatus(s as RuntimeStatus);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : failMessage);
    } finally {
      setBusy(false);
    }
  }

  const handleRepair = () => runAction(() => invoke("repair_runtime"), "Failed to repair runtime.");
  const handleDiagnose = () => runAction(() => invoke("run_diagnostics"), "Failed to run diagnostics.");
  const handleRollback = () => runAction(() => invoke("rollback_runtime"), "Failed to roll back runtime.");
  const handleUpdate = () =>
    runAction(
      () => invoke("apply_codex_update", { version: status.updateVersion }),
      "Failed to apply update."
    );

  const dash = "—";
  const versionLabel = status.installed ? status.version ?? dash : dash;
  const platformLabel = status.installed ? status.platform ?? dash : dash;
  const healthLabel = status.installed ? status.healthLabel ?? (status.healthy ? "100%" : dash) : dash;
  const healthColor = status.installed && status.healthy ? color.green : color.amber;

  const statRows: [string, string, string][] = [
    ["Health", healthLabel, healthColor],
    ["Mode", status.installed ? status.mode ?? dash : dash, color.secondary],
    ["Ownership", status.installed ? status.ownership ?? dash : dash, color.secondary],
    ["Install path", status.installed ? status.installPath ?? dash : dash, color.muted],
    ["Last update", status.installed ? status.lastUpdate ?? dash : dash, color.secondary],
    ["Uptime", status.installed ? status.uptime ?? dash : dash, color.secondary],
  ];

  const actionButtonStyle: React.CSSProperties = {
    background: color.ink3,
    borderRadius: radius.sm,
    padding: "8px 16px",
    border: `${hairline}px solid ${color.rule}`,
    fontSize: 12,
    color: color.primary,
    fontFamily: "inherit",
    cursor: busy ? "wait" : "pointer",
    opacity: busy ? 0.6 : 1,
  };

  return (
    <div style={{ display: "flex", height: "100%" }} role="main" aria-label="Codex runtime">
      {/* ── Left pane: managed runtime status ── */}
      <div
        aria-label="Managed runtime status"
        style={{
          width: size.codexLeftPane,
          padding: "40px 48px",
          borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex",
          flexDirection: "column",
          flexShrink: 0,
        }}
      >
        <SectionLabel textColor={color.dim}>MANAGED RUNTIME</SectionLabel>
        <div style={{ height: 10, flexShrink: 0 }} />
        <span style={{ ...font.version, color: color.primary }}>{versionLabel}</span>
        <span style={{ ...font.runtimeName, color: color.secondary, marginTop: 6 }}>
          {status.installed ? `Codex · ${platformLabel}` : "Codex not installed"}
        </span>

        <div style={{ height: 28, flexShrink: 0 }} />
        <HairlineRule />
        <div style={{ height: 24, flexShrink: 0 }} />

        <div>
          {statRows.map(([key, value, valueColor], i) => (
            <div key={key}>
              <div style={{ height: size.codexSpecRow, display: "flex", alignItems: "center" }}>
                <span style={{ fontSize: 12, color: color.muted, width: size.codexSpecKey, flexShrink: 0 }}>
                  {key}
                </span>
                <span style={{ ...font.captionStrong, color: valueColor }}>{value}</span>
              </div>
              {i < statRows.length - 1 && <HairlineRule opacity={ruleOpacity.spec} />}
            </div>
          ))}
        </div>

        <div style={{ height: 20, flexShrink: 0 }} />
        <div aria-label="Runtime actions" style={{ display: "flex", gap: 8 }}>
          <button
            type="button"
            onClick={handleRepair}
            disabled={busy}
            aria-label="Repair Codex runtime"
            style={actionButtonStyle}
          >
            Repair
          </button>
          <button
            type="button"
            onClick={handleDiagnose}
            disabled={busy}
            aria-label="Diagnose Codex runtime"
            style={actionButtonStyle}
          >
            Diagnose
          </button>
          <button
            type="button"
            onClick={handleRollback}
            disabled={busy}
            aria-label="Rollback Codex runtime"
            style={actionButtonStyle}
          >
            Rollback
          </button>
        </div>

        <div role="alert" aria-live="polite" style={{ minHeight: 16, marginTop: 12 }}>
          {error && <p style={{ fontSize: 12, color: color.danger, margin: 0 }}>{error}</p>}
        </div>
      </div>

      {/* ── Right pane: updates + history + diagnostics ── */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        {status.updateAvailable && status.updateVersion && (
          <div
            aria-label="Update available"
            style={{
              background: color.ink1,
              padding: "32px 40px 28px 40px",
              borderBottom: `${hairline}px solid ${color.rule}`,
              display: "flex",
              flexDirection: "column",
              flexShrink: 0,
            }}
          >
            <SectionLabel textColor={color.amber}>UPDATE AVAILABLE</SectionLabel>
            <div style={{ height: 8, flexShrink: 0 }} />
            <div style={{ display: "flex", alignItems: "flex-end", gap: 16 }}>
              <span style={{ ...font.versionCompare, color: color.dim }}>{status.version ?? dash}</span>
              <span style={{ fontSize: 28, color: color.muted }} aria-hidden="true">→</span>
              <span style={{ ...font.versionCompare, color: color.primary }}>{status.updateVersion}</span>
              <span
                style={{
                  borderRadius: radius.xs,
                  background: color.accentDim,
                  border: `${hairline}px solid ${color.rule}`,
                  padding: "3px 8px",
                  fontSize: 10,
                  fontWeight: 600,
                  letterSpacing: 1.5,
                  color: color.accent,
                }}
              >
                {status.updateChannel ?? "stable"}
              </span>
            </div>
            <div style={{ height: 16, flexShrink: 0 }} />
            <span style={{ fontSize: 12, color: color.muted }}>{status.updateMeta ?? ""}</span>
            <div style={{ height: 20, flexShrink: 0 }} />
            <div aria-label="Update actions" style={{ display: "flex", gap: 10 }}>
              <button
                type="button"
                onClick={handleUpdate}
                disabled={busy}
                aria-label={`Update to ${status.updateVersion}`}
                style={{
                  background: color.accent,
                  color: color.ink0,
                  border: "none",
                  borderRadius: radius.sm,
                  padding: "10px 20px",
                  fontSize: 14,
                  fontWeight: 700,
                  fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer",
                  opacity: busy ? 0.7 : 1,
                }}
              >
                Update to {status.updateVersion}
              </button>
              <button
                type="button"
                disabled={busy}
                aria-label="Skip this version"
                style={{
                  background: color.ink3,
                  border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm,
                  padding: "10px 18px",
                  fontSize: 13,
                  color: color.primary,
                  fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer",
                  opacity: busy ? 0.7 : 1,
                }}
              >
                Skip this version
              </button>
            </div>
          </div>
        )}

        <div aria-label="Version history" style={{ padding: "24px 40px", display: "flex", flexDirection: "column" }}>
          <SectionLabel textColor={color.dim}>VERSION HISTORY</SectionLabel>
          <div style={{ height: 12, flexShrink: 0 }} />
          <HairlineRule />
          {status.history.length === 0 && (
            <span style={{ fontSize: 12, color: color.muted, marginTop: 12 }}>{dash}</span>
          )}
          {status.history.map((entry, i) => (
            <div key={entry.version}>
              <div style={{ height: size.codexHistoryRow, display: "flex", alignItems: "center", gap: 16 }}>
                <span style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{entry.version}</span>
                <span style={{ fontSize: 11, color: color.muted }}>{entry.state}</span>
                <div style={{ flex: 1 }} />
                {entry.state.toLowerCase() !== "current" && (
                  <button
                    type="button"
                    onClick={() =>
                      runAction(
                        () => invoke("rollback_runtime", { version: entry.version }),
                        "Failed to restore version."
                      )
                    }
                    disabled={busy}
                    aria-label={`Restore version ${entry.version}`}
                    style={{
                      background: "none",
                      border: "none",
                      fontSize: 12,
                      color: color.secondary,
                      fontFamily: "inherit",
                      cursor: busy ? "wait" : "pointer",
                      padding: 0,
                    }}
                  >
                    Restore
                  </button>
                )}
              </div>
              {i < status.history.length - 1 && <HairlineRule opacity={ruleOpacity.list} />}
            </div>
          ))}
        </div>

        <div
          aria-label="Diagnostics"
          style={{
            padding: "20px 40px",
            borderTop: `${hairline}px solid ${color.rule}`,
            display: "flex",
            flexDirection: "column",
          }}
        >
          <SectionLabel textColor={color.dim}>DIAGNOSTICS</SectionLabel>
          <div style={{ height: 12, flexShrink: 0 }} />
          <HairlineRule />
          {status.diagnostics.length === 0 && (
            <span style={{ fontSize: 12, color: color.muted, marginTop: 12 }}>{dash}</span>
          )}
          {status.diagnostics.map((diag, i) => (
            <div key={diag.name}>
              <div style={{ height: size.codexSpecRow, display: "flex", alignItems: "center", gap: 16 }}>
                <span style={{ fontSize: 12, color: color.muted, width: size.codexSpecKey, flexShrink: 0 }}>
                  {diag.name}
                </span>
                <span style={{ ...font.captionStrong, color: RESULT_COLOR[diag.result] }}>
                  {diag.result}
                </span>
              </div>
              {i < status.diagnostics.length - 1 && <HairlineRule opacity={ruleOpacity.spec} />}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
