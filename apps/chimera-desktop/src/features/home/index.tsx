// Chimera++ 2.0 — Home feature (launch screen)
// First viewport is a thesis: shows system state, one primary action.
import { useState, useEffect } from "react";

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

export function HomeFeature() {
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
      setLaunchError(err instanceof Error ? err.message : "Failed to launch Codex. Check diagnostics.");
    } finally {
      setLaunching(false);
    }
  }

  const providerLabel = status.officialMode ? "Official Codex" : (status.providerName ?? "No provider");
  const hc: Record<string, string> = { unknown: "#5E5E5E", healthy: "#34C759", auth_failed: "#FF453A", unreachable: "#FF453A" };
  const healthColor = hc[status.providerHealth] ?? "#5E5E5E";

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {/* ── Hero zone ── */}
      <div style={{ flex: 1, display: "flex" }}>
        {/* Left: primary status statement */}
        <div role="main" aria-label="System status" style={{ flex: 1, display: "flex", flexDirection: "column", justifyContent: "center", padding: "0 64px", gap: 0 }}>
          <p style={{ fontSize: 11, fontWeight: 500, letterSpacing: 1.5, color: "#5E5E5E", margin: "0 0 10px" }}>ACTIVE PROVIDER</p>
          <h1 style={{ fontSize: 80, fontWeight: 700, color: "#EBEBEB", margin: 0, lineHeight: 0.9 }}>{providerLabel}</h1>
          <div style={{ display: "flex", alignItems: "center", gap: 14, marginTop: 12 }}>
            <span style={{ fontSize: 20, color: "#3A3A3A" }}>→</span>
            <span style={{ fontSize: 20, fontWeight: 400, color: "#5E5E5E" }}>{status.codexVersion ?? "Codex not installed"}</span>
            <span style={{ width: 1, height: 16, background: "#282828" }} />
            <span style={{ fontSize: 14, fontWeight: 500, color: status.codexRunning ? "#34C759" : "#5E5E5E" }}>
              {status.codexRunning ? "running" : "stopped"}
            </span>
          </div>
          <div style={{ display: "flex", gap: 28, marginTop: 20 }}>
            {[["Health", status.providerHealth.replace(/_/g, " "), healthColor], ["Provider", status.officialMode ? "Official" : "Custom", "#999"], ["Version", status.codexVersion ?? "—", "#999"]].map(([label, value, color]) => (
              <div key={label} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <span style={{ fontSize: 18, fontWeight: 600, color: color as string }}>{value}</span>
                <span style={{ fontSize: 10, color: "#3A3A3A" }}>{label}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Vertical divider */}
        <div style={{ width: 1, background: "#282828", alignSelf: "stretch" }} />

        {/* Right: primary action */}
        <div style={{ width: 300, display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center", gap: 14, padding: "0 40px" }}>
          <button
            onClick={handleLaunch} disabled={launching}
            aria-label={status.codexRunning ? "Codex is already running" : "Launch Codex"}
            style={{
              background: "#FF4D3D", color: "#0C0C0C", border: "none", borderRadius: 4,
              padding: "15px 36px", fontSize: 16, fontWeight: 700, cursor: launching ? "wait" : "pointer",
              opacity: launching ? 0.7 : 1, width: "100%",
            }}
          >
            {launching ? "Launching…" : status.codexRunning ? "Codex is running" : "Launch Codex"}
          </button>
          {launchError && (
            <p role="alert" style={{ fontSize: 12, color: "#FF453A", textAlign: "center", margin: 0 }}>
              {launchError}
            </p>
          )}
          <p style={{ fontSize: 11, color: "#3A3A3A", textAlign: "center", margin: 0 }}>⌘K  quick access</p>
        </div>
      </div>

      {/* ── Data strip ── */}
      <div style={{ borderTop: "1px solid #282828", display: "flex" }}>
        {[
          { title: "PROVIDER", rows: [["Endpoint", status.providerName ?? "Official"], ["Protocol", "OpenAI Responses"], ["Status", status.providerHealth]] },
          { title: "RUNTIME", rows: [["Version", status.codexVersion ?? "—"], ["Mode", "Managed Portable"], ["Health", status.codexRunning ? "Running" : "Stopped"]] },
          { title: "UPDATES", rows: [["Chimera", "Up to date"], ["Codex", "—"], ["Last check", "—"]] },
        ].map((col, i, arr) => (
          <div key={col.title} style={{ flex: 1, padding: "22px 32px", borderRight: i < arr.length - 1 ? "1px solid #282828" : "none" }}>
            <p style={{ fontSize: 10, fontWeight: 600, letterSpacing: 1.5, color: "#3A3A3A", margin: "0 0 8px" }}>{col.title}</p>
            <div style={{ height: 1, background: "#1C1C1C", marginBottom: 8 }} />
            {col.rows.map(([k, v]) => (
              <div key={k} style={{ display: "flex", height: 28, alignItems: "center" }}>
                <span style={{ fontSize: 12, color: "#5E5E5E", width: 130 }}>{k}</span>
                <span style={{ fontSize: 12, fontWeight: 500, color: "#666" }}>{v}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
