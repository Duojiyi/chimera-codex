import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { color, type as font, radius, hairline } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";

interface SystemStatus {
  providerName: string | null;
  providerHealth: "unknown" | "healthy" | "auth_failed" | "unreachable";
  codexVersion: string | null;
  codexRunning: boolean;
  officialMode: boolean;
}

const EMPTY_STATUS: SystemStatus = {
  providerName: null,
  providerHealth: "unknown",
  codexVersion: null,
  codexRunning: false,
  officialMode: true,
};

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

const HEALTH_LABEL_KEY: Record<SystemStatus["providerHealth"], TranslationKey> = {
  unknown: "health.unknown",
  healthy: "health.healthy",
  auth_failed: "health.authFailed",
  unreachable: "health.unreachable",
};

const HEALTH_COLOR: Record<SystemStatus["providerHealth"], string> = {
  unknown: color.amber,
  healthy: color.green,
  auth_failed: color.danger,
  unreachable: color.danger,
};

const HEALTH_TEXT_COLOR: Record<SystemStatus["providerHealth"], string> = {
  unknown: color.amberText,
  healthy: color.greenText,
  auth_failed: color.dangerText,
  unreachable: color.dangerText,
};

const panel: CSSProperties = {
  background: color.ink3,
  border: `${hairline}px solid ${color.rule}`,
  borderRadius: radius.md,
  boxShadow: "0 3px 10px rgba(25, 51, 58, 0.05)",
};

function Panel({ children, style, className }: { children: ReactNode; style?: CSSProperties; className?: string }) {
  return <section className={className} style={{ ...panel, ...style }}>{children}</section>;
}

function StatCard({ label, value, detail, tone = "default" }: { label: string; value: string; detail: string; tone?: "default" | "green" }) {
  return (
    <Panel className="home-stat-card" style={{ padding: "14px 16px", minHeight: 96, display: "flex", flexDirection: "column", justifyContent: "space-between" }}>
      <span style={{ ...font.captionStrong, color: color.muted }}>{label}</span>
      <div style={{ display: "flex", alignItems: "baseline", gap: 8, minWidth: 0 }}>
        <strong className="truncate-safe home-stat-value" title={value} style={{ fontSize: 24, lineHeight: 1, color: tone === "green" ? color.greenText : color.primary }}>{value}</strong>
        <span style={{ ...font.caption, color: color.muted }}>{detail}</span>
      </div>
    </Panel>
  );
}

function TrendChart({ running, health, trendLabel, weekdays }: { running: boolean; health: SystemStatus["providerHealth"]; trendLabel: string; weekdays: string[] }) {
  const points = running && health === "healthy" ? "0,70 54,54 108,63 162,38 216,45 270,22 324,30 378,10" : "0,68 54,60 108,66 162,52 216,58 270,44 324,52 378,42";
  return (
    <div style={{ position: "relative", height: 146, marginTop: 8, borderBottom: `${hairline}px solid ${color.rule}` }} aria-label={trendLabel}>
      <div style={{ position: "absolute", inset: "8px 0 26px", display: "grid", gridTemplateRows: "repeat(4, 1fr)" }} aria-hidden="true">
        {[0, 1, 2, 3].map((row) => <span key={row} style={{ borderTop: `${hairline}px solid ${color.rule}`, opacity: 0.65 }} />)}
      </div>
      <svg viewBox="0 0 378 90" preserveAspectRatio="none" style={{ position: "absolute", left: 0, right: 0, top: 10, width: "100%", height: 92, overflow: "visible" }} role="img" aria-label={trendLabel}>
        <polyline points={points} fill="none" stroke={color.green} strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
        <circle cx={running && health === "healthy" ? 378 : 378} cy={running && health === "healthy" ? 10 : 42} r="4" fill={color.green} />
      </svg>
      <div style={{ position: "absolute", left: 0, right: 0, bottom: 7, display: "flex", justifyContent: "space-between", ...font.caption, color: color.muted }}>
        {weekdays.map((day) => <span key={day}>{day}</span>)}
      </div>
    </div>
  );
}

function Calendar({ label, weekdays }: { label: string; weekdays: string[] }) {
  const days = Array.from({ length: 35 }, (_, index) => index - 3);
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(7, 1fr)", gap: 5, marginTop: 10 }} aria-label={label}>
      {weekdays.map((day, index) => <span key={`${day}-${index}`} style={{ ...font.captionStrong, color: color.muted, textAlign: "center", paddingBottom: 4 }}>{day}</span>)}
      {days.map((day, index) => {
        const active = day === 12;
        const update = day === 8 || day === 21;
        return <span key={index} style={{ aspectRatio: "1", display: "grid", placeItems: "center", borderRadius: radius.xs, background: active ? color.accent : update ? color.cardAlt : "transparent", color: active ? color.ink3 : day < 1 ? color.rule : color.secondary, fontSize: 11, fontWeight: active || update ? 700 : 400 }}>{day > 0 ? day : ""}</span>;
      })}
    </div>
  );
}

export function HomeFeature() {
  const { t } = useI18n();
  const [status, setStatus] = useState<SystemStatus>(EMPTY_STATUS);
  const [launching, setLaunching] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    invoke("get_system_status").then((raw) => {
      if (raw && typeof raw === "object") setStatus(raw as SystemStatus);
    }).catch(() => {});
  }, []);

  async function handleLaunch() {
    setLaunching(true);
    setMessage(null);
    try {
      await invoke("launch_codex");
      setStatus((current) => ({ ...current, codexRunning: true }));
      setMessage(t("home.alreadyRunning"));
    } catch (error: unknown) {
      setMessage(error instanceof Error ? error.message : t("home.launchFailed"));
    } finally {
      setLaunching(false);
    }
  }

  const provider = status.officialMode ? t("home.officialCodex") : status.providerName ?? t("home.noProvider");
  const health = t(HEALTH_LABEL_KEY[status.providerHealth]);
  const healthColor = HEALTH_COLOR[status.providerHealth];
  const healthTextColor = HEALTH_TEXT_COLOR[status.providerHealth];
  const version = status.codexVersion ?? t("common.dash");
  const weekdays = [t("home.dayMon"), t("home.dayTue"), t("home.dayWed"), t("home.dayThu"), t("home.dayFri"), t("home.daySat"), t("home.daySun")];
  const weekdaysShort = [t("home.dayMonShort"), t("home.dayTueShort"), t("home.dayWedShort"), t("home.dayThuShort"), t("home.dayFriShort"), t("home.daySatShort"), t("home.daySunShort")];

  return (
    <div role="main" aria-label={t("home.statusAriaLabel")} style={{ flex: 1, minWidth: 0, overflow: "auto", padding: 24, background: color.ink1 }}>
      <div style={{ display: "grid", gap: 14, maxWidth: 1020, margin: "0 auto" }}>
        <header style={{ minHeight: 64, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
          <div>
            <h1 style={{ fontSize: 28, lineHeight: 1.1, fontWeight: 700, color: color.primary, margin: 0 }}>{t("home.dashboardTitle")}</h1>
            <p style={{ ...font.caption, color: color.muted, margin: "5px 0 0" }}>{t("home.dashboardSubtitle")}</p>
          </div>
          <button type="button" onClick={handleLaunch} disabled={launching || status.codexRunning} aria-label={status.codexRunning ? t("home.launchAriaRunning") : t("home.launchAriaIdle")} style={{ minHeight: 38, padding: "0 16px", border: "none", borderRadius: radius.sm, background: color.accent, color: color.ink3, ...font.actionLabel, cursor: launching ? "wait" : "pointer" }}>
            {launching ? t("home.launching") : status.codexRunning ? t("home.alreadyRunning") : t("home.launch")}
          </button>
        </header>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 12 }}>
          <StatCard label={t("home.statProvider")} value={provider} detail={health} tone={status.providerHealth === "healthy" ? "green" : "default"} />
          <StatCard label={t("home.statRuntime")} value={version} detail={status.codexRunning ? t("home.running") : t("home.stopped")} />
          <StatCard label={t("home.statUpdates")} value={t("home.valUpToDate")} detail={t("home.statUpdatesDetail")} tone="green" />
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1.6fr) minmax(260px, 0.85fr)", gap: 14 }}>
          <Panel style={{ padding: 18, minHeight: 238 }}>
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between" }}>
              <div><h2 style={{ margin: 0, color: color.primary, fontSize: 18, lineHeight: 1.2 }}>{t("home.statusTitle")}</h2><p style={{ margin: "4px 0 0", ...font.caption, color: color.muted }}>{t("home.statusSubtitle")}</p></div>
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6, color: healthTextColor, ...font.captionStrong }}><i style={{ width: 7, height: 7, borderRadius: "50%", background: healthColor }} />{health}</span>
            </div>
            <TrendChart running={status.codexRunning} health={status.providerHealth} trendLabel={t("home.statusChartAriaLabel")} weekdays={weekdays} />
          </Panel>
          <Panel style={{ padding: 18, minHeight: 238, background: color.cardAlt }}>
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between" }}>
              <div><h2 style={{ margin: 0, color: color.primary, fontSize: 18, lineHeight: 1.2 }}>{t("home.scheduleTitle")}</h2><p style={{ margin: "4px 0 0", ...font.caption, color: color.muted }}>{t("home.scheduleSubtitle")}</p></div>
              <span aria-hidden="true" style={{ color: color.secondary, fontSize: 18 }}>···</span>
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: 8 }}><strong style={{ color: color.primary, fontSize: 16 }}>{t("home.scheduleMonth")}</strong><span style={{ ...font.caption, color: color.muted }}>{t("home.scheduleToday")}</span></div>
            <Calendar label={t("home.calendarAriaLabel")} weekdays={weekdaysShort} />
          </Panel>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1.1fr) minmax(0, 1fr)", gap: 14 }}>
          <Panel style={{ padding: 18, minHeight: 174 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}><div><h2 style={{ margin: 0, color: color.primary, fontSize: 18 }}>{t("home.providerCardTitle")}</h2><p style={{ margin: "4px 0 0", ...font.caption, color: color.muted }}>{t("home.providerCardSubtitle")}</p></div><span style={{ padding: "5px 8px", borderRadius: radius.pill, background: color.accentDim, color: healthTextColor, ...font.captionStrong }}>{health}</span></div>
            <div style={{ marginTop: 16, display: "grid", gridTemplateColumns: "110px 1fr", rowGap: 10, columnGap: 14 }}>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowEndpoint")}</span><strong style={{ ...font.captionStrong, color: color.secondary, overflow: "hidden", textOverflow: "ellipsis" }}>{status.officialMode ? t("home.valOfficial") : status.providerName ?? t("common.dash")}</strong>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowProtocol")}</span><strong style={{ ...font.captionStrong, color: color.secondary }}>{t("home.valResponses")}</strong>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowMode")}</span><strong style={{ ...font.captionStrong, color: color.secondary }}>{status.officialMode ? t("home.valOfficial") : t("home.valCustom")}</strong>
            </div>
          </Panel>
          <Panel style={{ padding: 18, minHeight: 174, background: color.ink3 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}><div><h2 style={{ margin: 0, color: color.primary, fontSize: 18 }}>{t("home.codexCardTitle")}</h2><p style={{ margin: "4px 0 0", ...font.caption, color: color.muted }}>{t("home.codexCardSubtitle")}</p></div><span style={{ color: status.codexRunning ? color.greenText : color.muted, ...font.captionStrong }}>{status.codexRunning ? t("home.running") : t("home.stopped")}</span></div>
            <div style={{ display: "grid", gridTemplateColumns: "110px 1fr", rowGap: 10, columnGap: 14, marginTop: 16 }}>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowVersion")}</span><strong className="wrap-safe home-runtime-version" style={{ ...font.captionStrong, color: color.secondary }}>{version}</strong>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowMode")}</span><strong style={{ ...font.captionStrong, color: color.secondary }}>{t("home.valManagedPortable")}</strong>
              <span style={{ ...font.caption, color: color.muted }}>{t("home.rowLastCheck")}</span><strong style={{ ...font.captionStrong, color: color.secondary }}>{t("home.statUpdatesDetail")}</strong>
            </div>
          </Panel>
        </div>

        <div role="status" aria-live="polite" style={{ minHeight: 18, color: message?.includes("失败") ? color.danger : color.muted, ...font.caption }}>{message}</div>
      </div>
    </div>
  );
}
