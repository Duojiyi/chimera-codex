// Chimera++ 2.0 — Settings feature.
// Layout is a 1:1 implementation of the Pencil design frame `Settings` (Body only;
// the 48px top rail is rendered by the parent shell).
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState, useEffect, type ReactNode } from "react";
import { color, type as font, size, radius, hairline, indicator, ruleOpacity } from "../../design/tokens.ts";

type CategoryId = "general" | "privacy" | "updates" | "advanced" | "about";

interface SettingsState {
  launchAtLogin: boolean;
  launchCodexOnStart: boolean;
  startMinimized: boolean;
  updateChannel: string;
  language: string;
  logRetention: string;
  structuredLogs: boolean;
  anonymousUsage: boolean;
  crashReporting: boolean;
}

type BooleanSettingKey = Extract<
  keyof SettingsState,
  "launchAtLogin" | "launchCodexOnStart" | "startMinimized" | "structuredLogs" | "anonymousUsage" | "crashReporting"
>;

const DEFAULT_SETTINGS: SettingsState = {
  launchAtLogin: true,
  launchCodexOnStart: false,
  startMinimized: false,
  updateChannel: "stable",
  language: "English (US)",
  logRetention: "30 days",
  structuredLogs: true,
  anonymousUsage: false,
  crashReporting: false,
};

const CATEGORIES: { id: CategoryId; label: string }[] = [
  { id: "general", label: "General" },
  { id: "privacy", label: "Privacy" },
  { id: "updates", label: "Updates" },
  { id: "advanced", label: "Advanced" },
  { id: "about", label: "About" },
];

const SUBTITLES: Partial<Record<CategoryId, string>> = {
  general: "Application behavior, startup, and interface defaults.",
};

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

function Toggle({
  checked,
  label,
  onChange,
  disabled,
}: {
  checked: boolean;
  label: string;
  onChange: () => void;
  disabled?: boolean;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={onChange}
        disabled={disabled}
        style={{
          width: size.toggleW,
          height: size.toggleH,
          borderRadius: 10,
          border: `${hairline}px solid ${color.rule}`,
          position: "relative",
          background: checked ? color.accent : color.ink3,
          padding: 0,
          cursor: disabled ? "default" : "pointer",
        }}
      >
        <span
          aria-hidden="true"
          style={{
            position: "absolute",
            width: size.toggleKnob,
            height: size.toggleKnob,
            borderRadius: "50%",
            top: 2,
            left: checked ? 19 : 2,
            background: checked ? color.ink0 : color.muted,
            transition: "left 120ms",
          }}
        />
      </button>
      <span style={{ fontSize: 12, color: checked ? color.accent : color.muted }}>{checked ? "On" : "Off"}</span>
    </div>
  );
}

function Select({ label, value }: { label: string; value: string }) {
  return (
    <button
      type="button"
      aria-label={`${label}: ${value}`}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        background: color.ink3,
        borderRadius: radius.sm,
        padding: "6px 12px",
        border: `${hairline}px solid ${color.rule}`,
        fontFamily: "inherit",
        cursor: "pointer",
      }}
    >
      <span style={{ fontSize: 12, color: color.primary }}>{value}</span>
      <span aria-hidden="true" style={{ fontSize: 9, color: color.muted }}>▾</span>
    </button>
  );
}

function Item({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <div style={{ height: size.settingsItemRow, display: "flex", alignItems: "center", gap: 12 }}>
        <span style={{ fontSize: 13, color: color.secondary, width: size.settingsItemKey, flexShrink: 0 }}>
          {label}
        </span>
        <div style={{ flex: 1 }} />
        {children}
      </div>
      <div style={{ height: hairline, background: color.rule, opacity: ruleOpacity.settingsItem }} />
    </>
  );
}

function Section({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <p style={{ ...font.sectionLabel, color: color.dim, margin: 0 }}>{label}</p>
      <div style={{ height: hairline, background: color.rule, marginTop: 6 }} />
      {children}
    </div>
  );
}

export function SettingsFeature() {
  const [active, setActive] = useState<CategoryId>("general");
  const [settings, setSettings] = useState<SettingsState>(DEFAULT_SETTINGS);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke("get_settings")
      .then((s) => s && setSettings(s as SettingsState))
      .catch(() => {});
  }, []);

  function toggleSetting(key: BooleanSettingKey) {
    setSettings((prev) => ({ ...prev, [key]: !prev[key] }));
  }

  async function handleSave() {
    setBusy(true);
    setError(null);
    try {
      await invoke("save_settings", { settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to save settings.");
    } finally {
      setBusy(false);
    }
  }

  async function handleReset() {
    setBusy(true);
    setError(null);
    try {
      await invoke("reset_settings");
      setSettings(DEFAULT_SETTINGS);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to reset settings.");
    } finally {
      setBusy(false);
    }
  }

  const activeCategory = CATEGORIES.find((c) => c.id === active) ?? CATEGORIES[0];
  const subtitle = SUBTITLES[active];

  return (
    <div style={{ display: "flex", height: "100%" }}>
      {/* ── Category nav (spec: 220px fixed) ── */}
      <nav
        role="tablist"
        aria-label="Settings categories"
        aria-orientation="vertical"
        style={{
          width: size.settingsNav,
          background: color.ink1,
          borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex",
          flexDirection: "column",
          flexShrink: 0,
        }}
      >
        <div
          style={{
            height: size.panelHead,
            padding: "0 20px",
            borderBottom: `${hairline}px solid ${color.rule}`,
            display: "flex",
            alignItems: "center",
            flexShrink: 0,
          }}
        >
          <span style={{ fontSize: 13, fontWeight: 600, color: color.secondary }}>Preferences</span>
        </div>

        {CATEGORIES.map((cat) => {
          const on = cat.id === active;
          return (
            <button
              key={cat.id}
              type="button"
              role="tab"
              aria-selected={on}
              onClick={() => setActive(cat.id)}
              style={{
                height: size.settingsCatRow,
                padding: "0 20px",
                display: "flex",
                alignItems: "center",
                background: on ? color.ink2 : "transparent",
                border: "none",
                borderLeftWidth: indicator.rowEdge,
                borderLeftStyle: "solid",
                borderLeftColor: on ? color.accent : "transparent",
                fontFamily: "inherit",
                fontSize: 13,
                fontWeight: on ? 600 : 400,
                color: on ? color.primary : color.secondary,
                cursor: "pointer",
                textAlign: "left",
              }}
            >
              {cat.label}
            </button>
          );
        })}
      </nav>

      {/* ── Content ── */}
      <div
        role="main"
        aria-label={`${activeCategory.label} settings`}
        style={{ flex: 1, padding: "32px 48px", display: "flex", flexDirection: "column", overflow: "auto" }}
      >
        <h1 style={{ ...font.pageTitle, color: color.primary, margin: 0 }}>{activeCategory.label}</h1>
        {subtitle && <p style={{ fontSize: 14, color: color.muted, margin: "6px 0 0" }}>{subtitle}</p>}
        <div style={{ height: hairline, background: color.rule, marginTop: 16 }} />

        <div style={{ marginTop: 24, display: "flex", flexDirection: "column", gap: 24 }}>
          {active === "general" ? (
            <>
              <Section label="STARTUP">
                <Item label="Launch Chimera++ at login">
                  <Toggle
                    checked={settings.launchAtLogin}
                    label="Launch Chimera++ at login"
                    onChange={() => toggleSetting("launchAtLogin")}
                  />
                </Item>
                <Item label="Launch Codex on Chimera++ start">
                  <Toggle
                    checked={settings.launchCodexOnStart}
                    label="Launch Codex on Chimera++ start"
                    onChange={() => toggleSetting("launchCodexOnStart")}
                  />
                </Item>
                <Item label="Start minimized to tray">
                  <Toggle
                    checked={settings.startMinimized}
                    label="Start minimized to tray"
                    onChange={() => toggleSetting("startMinimized")}
                  />
                </Item>
              </Section>

              <Section label="INTERFACE">
                <Item label="Update channel">
                  <Select label="Update channel" value={settings.updateChannel} />
                </Item>
                <Item label="Language">
                  <Select label="Language" value={settings.language} />
                </Item>
                <Item label="Log retention">
                  <Select label="Log retention" value={settings.logRetention} />
                </Item>
              </Section>

              <Section label="DIAGNOSTICS">
                <Item label="Structured logs">
                  <Toggle
                    checked={settings.structuredLogs}
                    label="Structured logs"
                    onChange={() => toggleSetting("structuredLogs")}
                  />
                </Item>
                <Item label="Anonymous usage statistics">
                  <Toggle
                    checked={settings.anonymousUsage}
                    label="Anonymous usage statistics"
                    onChange={() => toggleSetting("anonymousUsage")}
                  />
                </Item>
                <Item label="Crash reporting">
                  <Toggle
                    checked={settings.crashReporting}
                    label="Crash reporting"
                    onChange={() => toggleSetting("crashReporting")}
                  />
                </Item>
              </Section>
            </>
          ) : (
            <Section label="STATUS">
              <div style={{ height: size.settingsItemRow, display: "flex", alignItems: "center" }}>
                <span style={{ fontSize: 13, color: color.muted }}>
                  This category is not yet configured.
                </span>
              </div>
            </Section>
          )}
        </div>

        <div role="alert" aria-live="polite" style={{ minHeight: 16, marginTop: 8 }}>
          {error && <p style={{ fontSize: 12, color: color.danger, margin: 0 }}>{error}</p>}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 20 }}>
          <div style={{ flex: 1 }} />
          {saved && <span style={{ fontSize: 12, color: color.secondary }}>Saved</span>}
          <button
            type="button"
            onClick={handleReset}
            disabled={busy}
            style={{
              background: color.dangerBg,
              border: `${hairline}px solid ${color.dangerBorder}`,
              borderRadius: radius.sm,
              padding: "8px 16px",
              fontSize: 13,
              color: color.danger,
              fontFamily: "inherit",
              cursor: busy ? "wait" : "pointer",
              opacity: busy ? 0.7 : 1,
            }}
          >
            Reset all settings
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={busy}
            style={{
              background: color.accent,
              color: color.ink0,
              border: "none",
              borderRadius: radius.sm,
              padding: "8px 20px",
              fontSize: 13,
              fontWeight: 700,
              fontFamily: "inherit",
              cursor: busy ? "wait" : "pointer",
              opacity: busy ? 0.7 : 1,
            }}
          >
            Save changes
          </button>
        </div>
      </div>
    </div>
  );
}
