// Feature: codex — Managed Codex runtime status, updates, and diagnostics.
// Layout is a 1:1 implementation of the Pencil design frame `Codex` (Body).
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState, useEffect, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { color, type as font, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";
import {
  formatDownloadProgress,
  mergeUpdateCheck,
  type UpdateCheck,
} from "./lib/updateState.ts";

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

const RESULT_LABEL_KEY: Record<DiagnosticEntry["result"], TranslationKey> = {
  pass: "codex.diagPass",
  warn: "codex.diagWarn",
  fail: "codex.diagFail",
};

const MODE_LABEL_KEY: Record<string, TranslationKey> = {
  managed_portable: "codex.modeManagedPortable",
  external_msix: "codex.modeExternalMsix",
  external_portable: "codex.modeExternalPortable",
  none: "codex.modeNone",
};

function runtimeLabel(value: string | null, t: (key: TranslationKey) => string, fallback: string): string {
  return value && MODE_LABEL_KEY[value] ? t(MODE_LABEL_KEY[value]) : value ?? fallback;
}

function SectionLabel({ children, textColor }: { children: string; textColor: string }) {
  return (
    <p style={{ ...font.sectionLabel, color: textColor, margin: 0 }}>{children}</p>
  );
}

function HairlineRule({ opacity = 1 }: { opacity?: number }) {
  return <div style={{ height: hairline, background: color.rule, opacity, flexShrink: 0 }} />;
}

function SegmentedControl<T extends string>({
  label,
  value,
  options,
  disabled,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  disabled: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div role="group" aria-label={label} style={{ display: "flex", padding: 3, gap: 2, background: color.ink2, borderRadius: radius.sm }}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            disabled={disabled}
            onClick={() => onChange(option.value)}
            style={{
              minHeight: 30,
              padding: "0 12px",
              border: "none",
              borderRadius: radius.xs,
              background: selected ? color.ink3 : "transparent",
              color: selected ? color.primary : color.muted,
              fontFamily: "inherit",
              fontSize: 12,
              fontWeight: selected ? 600 : 400,
              cursor: disabled ? "wait" : "pointer",
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export function CodexFeature() {
  const { t, tf } = useI18n();
  const [status, setStatus] = useState<RuntimeStatus>(EMPTY_STATUS);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [updateSource, setUpdateSource] = useState<"auto" | "mirror">("auto");
  const [installMode, setInstallMode] = useState<"standard" | "portable">("standard");
  const [downloadProgress, setDownloadProgress] = useState({ percent: 0, label: "0%" });

  useEffect(() => {
    let cancelled = false;
    let stopListening: (() => void) | undefined;
    void listen<{ downloaded: number; total: number }>("codex://download-progress", (event) => {
      if (!cancelled) setDownloadProgress(formatDownloadProgress(event.payload.downloaded, event.payload.total));
    }).then((unlisten) => { stopListening = unlisten; });
    void Promise.all([invoke("get_runtime_status"), invoke("get_settings")]).then(([runtime, rawSettings]) => {
      if (cancelled) return;
      if (runtime) setStatus(runtime as RuntimeStatus);
      const settings = rawSettings as {
        checkCodexUpdatesOnStart?: boolean;
        codexUpdateSource?: "auto" | "mirror";
        codexInstallMode?: "standard" | "portable";
      } | undefined;
      const source = settings?.codexUpdateSource ?? "auto";
      const mode = settings?.codexInstallMode ?? "standard";
      setUpdateSource(source);
      setInstallMode(mode);
      if (settings?.checkCodexUpdatesOnStart !== false) void checkForUpdates(source, mode);
    }).catch(() => {});
    return () => {
      cancelled = true;
      stopListening?.();
    };
  }, []);

  async function checkForUpdates(
    source: "auto" | "mirror" = updateSource,
    mode: "standard" | "portable" = installMode,
  ) {
    setChecking(true);
    setError(null);
    try {
      const update = await invoke("check_codex_update", { source, installMode: mode }) as UpdateCheck | undefined;
      if (update) setStatus((current) => mergeUpdateCheck(current, update));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("codex.errCheckUpdate"));
    } finally {
      setChecking(false);
    }
  }

  async function runAction(action: () => Promise<unknown>, failMessage: string) {
    setBusy(true);
    setDownloadProgress({ percent: 0, label: "0%" });
    setError(null);
    setActionMessage(null);
    try {
      const result = await action() as { message?: string; actualMode?: string } | undefined;
      const s = await invoke("get_runtime_status").catch(() => undefined);
      if (s) setStatus(s as RuntimeStatus);
      setActionMessage(result?.message ?? t("codex.actionCompleted"));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : failMessage);
    } finally {
      setBusy(false);
    }
  }

  const handleRepair = () => runAction(
    () => invoke("repair_runtime", { source: updateSource, installMode }),
    t("codex.errRepair"),
  );
  const handleDiagnose = async () => {
    setBusy(true);
    setError(null);
    setActionMessage(null);
    try {
      const diagnostics = await invoke("run_diagnostics") as RuntimeStatus["diagnostics"];
      setStatus((current) => ({ ...current, diagnostics }));
      setActionMessage(t("codex.diagnosticsCompleted"));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("codex.errDiagnose"));
    } finally {
      setBusy(false);
    }
  };
  const handleRollback = () => runAction(() => invoke("rollback_runtime"), t("codex.errRollback"));
  const handleUninstall = () => {
    if (!window.confirm(t("codex.uninstallConfirm"))) return;
    void runAction(() => invoke("uninstall_codex"), t("codex.errUninstall"));
  };
  const handleUpdate = () =>
    runAction(
      () => invoke("apply_codex_update", { version: status.updateVersion, source: updateSource, installMode }),
      t("codex.errUpdate")
    );

  const dash = t("common.dash");
  const versionLabel = status.installed ? status.version ?? dash : dash;
  const platformLabel = status.installed ? status.platform ?? dash : dash;
  const healthLabel = status.installed ? status.healthLabel ?? (status.healthy ? "100%" : dash) : dash;
  const healthColor = status.installed && status.healthy ? color.green : color.amber;
  const managed = status.ownership === "chimera_verified";
  const hasRollback = managed && status.history.some((entry) => entry.state === "previous");

  const statRows: [string, string, string][] = [
    [t("codex.specHealth"), healthLabel, healthColor],
    [t("codex.specMode"), status.installed ? runtimeLabel(status.mode, t, dash) : dash, color.secondary],
    [t("codex.specOwnership"), status.installed ? (managed ? t("codex.ownershipVerified") : t("codex.ownershipNotOwned")) : dash, managed ? color.green : color.amber],
    [t("codex.specInstallPath"), status.installed ? status.installPath ?? dash : dash, color.muted],
    [t("codex.specLastUpdate"), status.installed ? status.lastUpdate ?? dash : dash, color.secondary],
    [t("codex.specUptime"), status.installed ? status.uptime ?? dash : dash, color.secondary],
  ];

  const actionButtonStyle: CSSProperties = {
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
    <div style={{ display: "flex", height: "100%" }} role="main" aria-label={t("codex.runtimeAriaLabel")}>
      {/* ── Left pane: managed runtime status ── */}
      <div
        aria-label={t("codex.managedRuntimeAriaLabel")}
        style={{
          width: size.codexLeftPane,
          padding: "40px 48px",
          borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex",
          flexDirection: "column",
          flexShrink: 0,
        }}
      >
        <SectionLabel textColor={color.dim}>{t("codex.managedRuntime")}</SectionLabel>
        <div style={{ height: 10, flexShrink: 0 }} />
        <span style={{ ...font.version, color: color.primary }}>{versionLabel}</span>
        <span style={{ ...font.runtimeName, color: color.secondary, marginTop: 6 }}>
          {status.installed ? tf("codex.runtimeLine", [platformLabel]) : t("codex.notInstalledFull")}
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
        <div aria-label={t("codex.runtimeActionsAriaLabel")} style={{ display: "flex", gap: 8 }}>
          <button
            type="button"
            onClick={handleRepair}
            disabled={busy || !status.installed}
            aria-label={t("codex.repairAriaLabel")}
            style={actionButtonStyle}
          >
            {busy ? t("common.loading") : t("codex.repair")}
          </button>
          <button
            type="button"
            onClick={() => void handleDiagnose()}
            disabled={busy}
            aria-label={t("codex.diagnoseAriaLabel")}
            style={actionButtonStyle}
          >
            {busy ? t("common.loading") : t("codex.diagnose")}
          </button>
          <button
            type="button"
            onClick={handleRollback}
            disabled={busy || !hasRollback}
            aria-label={t("codex.rollbackAriaLabel")}
            style={actionButtonStyle}
          >
            {busy ? t("common.loading") : t("codex.rollback")}
          </button>
          <button
            type="button"
            onClick={handleUninstall}
            disabled={busy || !status.installed}
            aria-label={t("codex.uninstallAriaLabel")}
            style={{ ...actionButtonStyle, color: color.danger }}
          >
            {busy ? t("common.loading") : t("codex.uninstall")}
          </button>
        </div>

        <div role="alert" aria-live="polite" style={{ minHeight: 16, marginTop: 12 }}>
          {error && <p style={{ fontSize: 12, color: color.danger, margin: 0 }}>{error}</p>}
          {!error && actionMessage && <p style={{ fontSize: 12, color: color.green, margin: 0 }}>{actionMessage}</p>}
        </div>
      </div>

      {/* ── Right pane: updates + history + diagnostics ── */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        <div
          style={{
            padding: "22px 40px",
            borderBottom: `${hairline}px solid ${color.rule}`,
            display: "flex",
            flexDirection: "column",
            gap: 14,
            background: color.ink1,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 18, flexWrap: "wrap" }}>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <span style={{ fontSize: 11, color: color.dim }}>{t("codex.updateSource")}</span>
              <SegmentedControl
                label={t("codex.updateSource")}
                value={updateSource}
                disabled={busy || checking}
                options={[
                  { value: "auto", label: t("codex.sourceAuto") },
                  { value: "mirror", label: t("codex.sourceMirror") },
                ]}
                onChange={(value) => { setUpdateSource(value); void checkForUpdates(value, installMode); }}
              />
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <span style={{ fontSize: 11, color: color.dim }}>{t("codex.installMode")}</span>
              <SegmentedControl
                label={t("codex.installMode")}
                value={installMode}
                disabled={busy || checking}
                options={[
                  { value: "standard", label: t("codex.modeStandard") },
                  { value: "portable", label: t("codex.modePortable") },
                ]}
                onChange={(value) => setInstallMode(value)}
              />
            </div>
            <div style={{ flex: 1 }} />
            <button
              type="button"
              onClick={() => void checkForUpdates()}
              disabled={busy || checking}
              style={{ ...actionButtonStyle, minHeight: 36 }}
            >
              {checking ? t("codex.checkingUpdate") : t("codex.checkUpdate")}
            </button>
          </div>
          {busy && (
            <div aria-live="polite" style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <div style={{ height: 4, flex: 1, background: color.ink3, overflow: "hidden", borderRadius: radius.xs }}>
                <div style={{ width: `${downloadProgress.percent}%`, height: "100%", background: color.accent, transition: "width 180ms ease-out" }} />
              </div>
              <span style={{ width: 36, fontSize: 11, color: color.secondary, textAlign: "right" }}>{downloadProgress.label}</span>
            </div>
          )}
        </div>

        {status.updateAvailable && status.updateVersion && (
          <div
            aria-label={t("codex.updateAvailableAriaLabel")}
            style={{
              background: color.ink1,
              padding: "32px 40px 28px 40px",
              borderBottom: `${hairline}px solid ${color.rule}`,
              display: "flex",
              flexDirection: "column",
              flexShrink: 0,
            }}
          >
            <SectionLabel textColor={color.amber}>{t("codex.updateAvailable")}</SectionLabel>
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
                {status.updateChannel ?? t("codex.channelStable")}
              </span>
            </div>
            <div style={{ height: 16, flexShrink: 0 }} />
            <span style={{ fontSize: 12, color: color.muted }}>{status.updateMeta ?? ""}</span>
            <div style={{ height: 20, flexShrink: 0 }} />
            <div aria-label={t("codex.updateActionsAriaLabel")} style={{ display: "flex", gap: 10 }}>
              <button
                type="button"
                onClick={handleUpdate}
                disabled={busy}
                aria-label={tf("codex.updateToVersion", [status.updateVersion])}
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
                {tf("codex.updateToVersion", [status.updateVersion])}
              </button>
              <button
                type="button"
                disabled={busy}
                aria-label={t("codex.skipVersionAriaLabel")}
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
                {t("codex.skipVersion")}
              </button>
            </div>
          </div>
        )}

        <div aria-label={t("codex.versionHistoryAriaLabel")} style={{ padding: "24px 40px", display: "flex", flexDirection: "column" }}>
          <SectionLabel textColor={color.dim}>{t("codex.versionHistory")}</SectionLabel>
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
                {entry.state === "previous" && (
                  <button
                    type="button"
                    onClick={() =>
                      runAction(
                        () => invoke("rollback_runtime", { version: entry.version }),
                        t("codex.errRestoreVersion")
                      )
                    }
                    disabled={busy}
                    aria-label={tf("codex.restoreVersionAriaLabel", [entry.version])}
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
                    {t("codex.restore")}
                  </button>
                )}
              </div>
              {i < status.history.length - 1 && <HairlineRule opacity={ruleOpacity.list} />}
            </div>
          ))}
        </div>

        <div
          aria-label={t("codex.diagnostics")}
          style={{
            padding: "20px 40px",
            borderTop: `${hairline}px solid ${color.rule}`,
            display: "flex",
            flexDirection: "column",
          }}
        >
          <SectionLabel textColor={color.dim}>{t("codex.diagnostics")}</SectionLabel>
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
                  {t(RESULT_LABEL_KEY[diag.result])}
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
