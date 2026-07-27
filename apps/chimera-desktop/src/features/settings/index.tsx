// Chimera++ 2.0 — Settings feature.
// Layout is a 1:1 implementation of the Pencil design frame `Settings` (Body only;
// the 48px top rail is rendered by the parent shell).
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState, useEffect, type ReactNode } from "react";
import { color, type as font, size, radius, hairline, indicator, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey, type Language } from "../../i18n/index.tsx";

type CategoryId = "general" | "privacy" | "updates" | "advanced" | "about";

interface SettingsState {
  launchAtLogin: boolean;
  launchCodexOnStart: boolean;
  startMinimized: boolean;
  updateChannel: string;
  logRetention: string;
  structuredLogs: boolean;
  anonymousUsage: boolean;
  crashReporting: boolean;
  checkCodexUpdatesOnStart: boolean;
  codexUpdateSource: "auto" | "mirror";
  codexInstallMode: "standard" | "portable";
}

type BooleanSettingKey = Extract<
  keyof SettingsState,
  "launchAtLogin" | "launchCodexOnStart" | "startMinimized" | "structuredLogs" | "anonymousUsage" | "crashReporting" | "checkCodexUpdatesOnStart"
>;

const DEFAULT_SETTINGS: SettingsState = {
  launchAtLogin: true,
  launchCodexOnStart: false,
  startMinimized: false,
  updateChannel: "stable",
  logRetention: "30 days",
  structuredLogs: true,
  anonymousUsage: false,
  crashReporting: false,
  checkCodexUpdatesOnStart: true,
  codexUpdateSource: "auto",
  codexInstallMode: "standard",
};

// Module-level constants hold i18n KEYS, never translated text — translation
// happens at render so a language switch re-renders without reloading the
// webview. scripts/verify-i18n.mjs enforces this.
const CATEGORIES: { id: CategoryId; labelKey: TranslationKey }[] = [
  { id: "general", labelKey: "settings.catGeneral" },
  { id: "privacy", labelKey: "settings.catPrivacy" },
  { id: "updates", labelKey: "settings.catUpdates" },
  { id: "advanced", labelKey: "settings.catAdvanced" },
  { id: "about", labelKey: "settings.catAbout" },
];

const SUBTITLES: Partial<Record<CategoryId, TranslationKey>> = {
  general: "settings.generalSubtitle",
  updates: "settings.updatesSubtitle",
};

// Internal setting values are plain codes (never shown directly); these maps
// hold the i18n KEY used to display each code, not the translated text itself.
const CHANNEL_LABEL_KEYS: Record<string, TranslationKey> = {
  stable: "settings.channelStable",
  beta: "settings.channelBeta",
};

const RETENTION_LABEL_KEYS: Record<string, TranslationKey> = {
  "7 days": "settings.retention7",
  "30 days": "settings.retention30",
  "90 days": "settings.retention90",
};

// Language endonyms — deliberately NOT translated (a "简体中文" reader should
// never see "Simplified Chinese" and an English reader should never see
// "英语"), so these are hardcoded rather than routed through the dictionary.
const LANGUAGE_OPTIONS: { value: Language; label: string }[] = [
  { value: "zh", label: "简体中文" },
  { value: "en", label: "English" },
];

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

/** Human-readable size. Never shown as raw bytes — "1073741824" tells nobody anything. */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

interface CleanupPlan {
  entries: { label: string; bytes: number }[];
  totalBytes: number;
  leavesKeychainEntries: boolean;
}

/**
 * What a portable install cannot do, and how to remove it.
 *
 * Both halves of Spec §10 live here because they are the same fact seen twice:
 * Chimera registers no Apps & Features entry, so it has to state that plainly
 * AND provide the removal path that absence implies. Shipping one without the
 * other would leave a user with no way to uninstall and no explanation.
 */
function PortablePanel({ mode }: { mode: "limits" | "cleanup" }) {
  const { t, tf } = useI18n();
  const [limitKeys, setLimitKeys] = useState<string[]>([]);
  const [plan, setPlan] = useState<CleanupPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    if (mode === "limits") {
      invoke("get_portable_limitations")
        .then((r) => setLimitKeys(((r ?? []) as { detailKey: string }[]).map((l) => l.detailKey)))
        .catch(() => {});
    } else {
      invoke("get_cleanup_plan")
        .then((r) => r && setPlan(r as CleanupPlan))
        .catch(() => {});
    }
  }, [mode]);

  async function handleCleanup() {
    // window.confirm rather than a custom dialog: this is irreversible, and a
    // native modal is the one control a user cannot mistake for decoration.
    if (!window.confirm(t("portable.cleanupConfirm"))) return;
    setBusy(true);
    setStatus(null);
    try {
      await invoke("execute_cleanup");
      setStatus(t("portable.cleanupDone"));
      setPlan({ entries: [], totalBytes: 0, leavesKeychainEntries: true });
    } catch (err: unknown) {
      setStatus(tf("portable.cleanupFailed", [err instanceof Error ? err.message : String(err)]));
    } finally {
      setBusy(false);
    }
  }

  if (mode === "limits") {
    return (
      <Section label={t("portable.sectionLimits")}>
        <p style={{ fontSize: 12, color: color.secondary, margin: "4px 0 12px", lineHeight: 1.6 }}>
          {t("portable.intro")}
        </p>
        <ul style={{ margin: 0, paddingLeft: 18, display: "grid", gap: 8 }}>
          {limitKeys.map((key) => (
            <li key={key} style={{ fontSize: 12, color: color.muted, lineHeight: 1.6 }}>
              {t(key as TranslationKey)}
            </li>
          ))}
        </ul>
      </Section>
    );
  }

  return (
    <Section label={t("portable.sectionCleanup")}>
      <p style={{ fontSize: 13, color: color.primary, margin: "4px 0 4px" }}>
        {t("portable.cleanupTitle")}
      </p>
      <p style={{ fontSize: 12, color: color.secondary, margin: "0 0 12px", lineHeight: 1.6 }}>
        {t("portable.cleanupDescription")}
      </p>

      {plan && plan.entries.length === 0 ? (
        <p style={{ fontSize: 12, color: color.muted, margin: "0 0 12px" }}>
          {t("portable.cleanupEmpty")}
        </p>
      ) : (
        <ul
          aria-label={t("portable.cleanupAriaLabel")}
          style={{ margin: "0 0 10px", padding: 0, listStyle: "none", display: "grid", gap: 4 }}
        >
          {(plan?.entries ?? []).map((e) => (
            <li
              key={e.label}
              style={{
                display: "flex",
                justifyContent: "space-between",
                fontSize: 12,
                color: color.muted,
              }}
            >
              <span>{e.label}</span>
              <span>{formatBytes(e.bytes)}</span>
            </li>
          ))}
        </ul>
      )}

      {plan && plan.totalBytes > 0 && (
        <p style={{ fontSize: 12, color: color.secondary, margin: "0 0 12px" }}>
          {tf("portable.cleanupTotal", [formatBytes(plan.totalBytes)])}
        </p>
      )}

      {/* Stated even when the list is empty: a user who already cleaned up
          still needs to know their keys were never part of it. */}
      {plan?.leavesKeychainEntries && (
        <p style={{ fontSize: 12, color: color.muted, margin: "0 0 14px", lineHeight: 1.6 }}>
          {t("portable.cleanupKeychainNote")}
        </p>
      )}

      <button
        type="button"
        onClick={handleCleanup}
        disabled={busy || !plan || plan.entries.length === 0}
        style={{
          background: color.dangerBg,
          border: `${hairline}px solid ${color.dangerBorder}`,
          borderRadius: radius.sm,
          padding: "8px 16px",
          fontSize: 13,
          color: color.dangerText,
          fontFamily: "inherit",
          cursor: busy ? "wait" : "pointer",
          opacity: busy || !plan || plan.entries.length === 0 ? 0.5 : 1,
        }}
      >
        {t("portable.cleanupButton")}
      </button>

      <div role="status" aria-live="polite" style={{ minHeight: 16, marginTop: 10 }}>
        {status && <span style={{ fontSize: 12, color: color.secondary }}>{status}</span>}
      </div>
    </Section>
  );
}

interface MigrationCandidate {
  source: string;
  sourceId: string;
  displayName: string;
  host: string;
  hasKey: boolean;
}

interface MigrationPreview {
  candidates: MigrationCandidate[];
  droppedFeatures: string[];
  warnings: string[];
}

/**
 * Import providers from Chimera++ 1.x and CC Switch.
 *
 * Preview and run are separate commands on purpose, and the UI keeps them
 * separate too: the preview only reads, so it can run on every visit, and it is
 * what the user actually approves. Nothing is written until they press the
 * button.
 */
function MigrationPanel() {
  const { t, tf } = useI18n();
  const [preview, setPreview] = useState<MigrationPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    invoke("preview_migration")
      .then((r) => r && setPreview(r as MigrationPreview))
      .catch(() => {});
  }, []);

  async function handleMigrate() {
    if (!window.confirm(t("migration.confirm"))) return;
    setBusy(true);
    setStatus(null);
    try {
      const r = (await invoke("run_migration")) as
        | { migrated: number; alreadyMigrated: number; skipped: { sourceId: string; reason: string }[] }
        | undefined;
      if (!r) return;
      setStatus(tf("migration.done", [r.migrated, r.alreadyMigrated, r.skipped.length]));
      // Re-read rather than assuming: a skipped candidate is still in the
      // source and should keep showing, with its reason.
      const next = (await invoke("preview_migration")) as MigrationPreview | undefined;
      if (next) setPreview(next);
    } catch (err: unknown) {
      setStatus(err instanceof Error ? err.message : t("migration.failed"));
    } finally {
      setBusy(false);
    }
  }

  const found = preview?.candidates ?? [];

  return (
    <Section label={t("migration.section")}>
      {found.length === 0 ? (
        <p style={{ fontSize: 12, color: color.muted, margin: "4px 0 0", lineHeight: 1.6 }}>
          {t("migration.nothingFound")}
        </p>
      ) : (
        <>
          <p style={{ fontSize: 12, color: color.secondary, margin: "4px 0 12px", lineHeight: 1.6 }}>
            {tf("migration.foundCount", [found.length])}
          </p>
          <ul
            aria-label={t("migration.listAriaLabel")}
            style={{ margin: "0 0 12px", padding: 0, listStyle: "none", display: "grid", gap: 6 }}
          >
            {found.map((c) => (
              <li
                key={`${c.source}:${c.sourceId}`}
                style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: color.muted }}
              >
                <span>{c.displayName} · {c.host}</span>
                <span style={{ color: c.hasKey ? color.secondary : color.danger }}>
                  {c.hasKey ? t("migration.withKey") : t("migration.withoutKey")}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}

      {/* Stated before migrating, not discovered afterwards. */}
      {(preview?.droppedFeatures.length ?? 0) > 0 && (
        <>
          <p style={{ fontSize: 12, color: color.secondary, margin: "0 0 6px" }}>
            {t("migration.droppedIntro")}
          </p>
          <ul style={{ margin: "0 0 12px", paddingLeft: 18, display: "grid", gap: 4 }}>
            {preview?.droppedFeatures.map((d) => (
              <li key={d} style={{ fontSize: 12, color: color.muted, lineHeight: 1.6 }}>{d}</li>
            ))}
          </ul>
        </>
      )}

      {(preview?.warnings.length ?? 0) > 0 && (
        <ul style={{ margin: "0 0 12px", paddingLeft: 18, display: "grid", gap: 4 }}>
          {preview?.warnings.map((w) => (
            <li key={w} style={{ fontSize: 12, color: color.danger, lineHeight: 1.6 }}>{w}</li>
          ))}
        </ul>
      )}

      <button
        type="button"
        onClick={handleMigrate}
        disabled={busy || found.length === 0}
        style={{
          background: color.accent,
          color: color.ink0,
          border: "none",
          borderRadius: radius.sm,
          padding: "8px 18px",
          fontSize: 13,
          fontWeight: 700,
          fontFamily: "inherit",
          cursor: busy ? "wait" : "pointer",
          opacity: busy || found.length === 0 ? 0.5 : 1,
        }}
      >
        {t("migration.run")}
      </button>

      <div role="status" aria-live="polite" style={{ minHeight: 16, marginTop: 10 }}>
        {status && <span style={{ fontSize: 12, color: color.secondary }}>{status}</span>}
      </div>
    </Section>
  );
}

function Toggle({
  checked,
  label,
  onLabel,
  offLabel,
  onChange,
  disabled,
}: {
  checked: boolean;
  label: string;
  onLabel: string;
  offLabel: string;
  onChange: () => void;
  disabled?: boolean;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <button
        className="settings-toggle"
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={onChange}
        disabled={disabled}
        style={{
          width: 44,
          height: 36,
          border: "none",
          position: "relative",
          background: "transparent",
          padding: 0,
          cursor: disabled ? "default" : "pointer",
          display: "grid",
          placeItems: "center",
        }}
      >
        <span
          aria-hidden="true"
          style={{
            width: size.toggleW,
            height: size.toggleH,
            borderRadius: 10,
            border: `${hairline}px solid ${color.rule}`,
            position: "relative",
            background: checked ? color.accent : color.ink3,
            boxSizing: "border-box",
          }}
        >
          <span
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
        </span>
      </button>
      <span style={{ fontSize: 12, color: checked ? color.accent : color.muted }}>
        {checked ? onLabel : offLabel}
      </span>
    </div>
  );
}

function Select({ ariaLabel, value }: { ariaLabel: string; value: string }) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
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

/**
 * The Language row's control. Visually it is byte-identical to `Select`
 * (same token references: ink3 / radius.sm / hairline / rule / primary /
 * muted) — that visible chip is `aria-hidden` and purely decorative. A real
 * `<select>` is layered transparently on top so the control is a genuine,
 * keyboard-operable form element: Tab focuses it, arrow keys / typing change
 * the value, and screen readers get the native select semantics instead of a
 * fake button.
 */
function LanguageSelect({
  ariaLabel,
  value,
  options,
  onChange,
}: {
  ariaLabel: string;
  value: Language;
  options: { value: Language; label: string }[];
  onChange: (next: Language) => void;
}) {
  const current = options.find((o) => o.value === value) ?? options[0];
  return (
    <div style={{ position: "relative", display: "inline-flex" }}>
      <div
        aria-hidden="true"
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          background: color.ink3,
          borderRadius: radius.sm,
          padding: "6px 12px",
          border: `${hairline}px solid ${color.rule}`,
          fontFamily: "inherit",
        }}
      >
        <span style={{ fontSize: 12, color: color.primary }}>{current.label}</span>
        <span style={{ fontSize: 9, color: color.muted }}>▾</span>
      </div>
      <select
        aria-label={ariaLabel}
        value={value}
        onChange={(e) => onChange(e.target.value as Language)}
        style={{
          position: "absolute",
          inset: 0,
          width: "100%",
          height: "100%",
          opacity: 0,
          border: "none",
          background: "transparent",
          cursor: "pointer",
        }}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
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

function SegmentedSetting<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div role="group" aria-label={label} style={{ display: "flex", gap: 2, padding: 3, background: color.ink2, borderRadius: radius.sm }}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option.value)}
            style={{
              minWidth: 92,
              minHeight: 30,
              padding: "0 12px",
              border: "none",
              borderRadius: radius.xs,
              background: selected ? color.ink3 : "transparent",
              color: selected ? color.primary : color.muted,
              fontFamily: "inherit",
              fontSize: 12,
              fontWeight: selected ? 600 : 400,
              cursor: "pointer",
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export function SettingsFeature() {
  const { lang, t, tf, setLang } = useI18n();
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
      setError(err instanceof Error ? err.message : t("settings.saveFailed"));
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
      setError(err instanceof Error ? err.message : t("settings.resetFailed"));
    } finally {
      setBusy(false);
    }
  }

  const activeCategory = CATEGORIES.find((c) => c.id === active) ?? CATEGORIES[0];
  const subtitle = SUBTITLES[active];

  return (
    <div className="settings-feature" style={{ display: "flex", height: "100%" }}>
      {/* ── Category nav (spec: 220px fixed) ── */}
      <nav
        role="tablist"
        aria-label={t("settings.navAriaLabel")}
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
          <span style={{ fontSize: 13, fontWeight: 600, color: color.secondary }}>{t("settings.preferences")}</span>
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
              {t(cat.labelKey)}
            </button>
          );
        })}
      </nav>

      {/* ── Content ── */}
      <div
        className="settings-content"
        role="main"
        aria-label={tf("settings.categoryAriaLabel", [t(activeCategory.labelKey)])}
        style={{ flex: 1, padding: "32px 40px", display: "flex", flexDirection: "column", overflow: "auto" }}
      >
        <h1 style={{ ...font.pageTitle, color: color.primary, margin: 0 }}>{t(activeCategory.labelKey)}</h1>
        {subtitle && <p style={{ fontSize: 14, color: color.muted, margin: "6px 0 0" }}>{t(subtitle)}</p>}
        <div style={{ height: hairline, background: color.rule, marginTop: 16 }} />

        <div style={{ marginTop: 24, display: "flex", flexDirection: "column", gap: 24 }}>
          {active === "general" ? (
            <>
              <Section label={t("settings.secStartup")}>
                <Item label={t("settings.launchAtLogin")}>
                  <Toggle
                    checked={settings.launchAtLogin}
                    label={t("settings.launchAtLogin")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("launchAtLogin")}
                  />
                </Item>
                <Item label={t("settings.launchCodexOnStart")}>
                  <Toggle
                    checked={settings.launchCodexOnStart}
                    label={t("settings.launchCodexOnStart")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("launchCodexOnStart")}
                  />
                </Item>
                <Item label={t("settings.startMinimized")}>
                  <Toggle
                    checked={settings.startMinimized}
                    label={t("settings.startMinimized")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("startMinimized")}
                  />
                </Item>
              </Section>

              <Section label={t("settings.secInterface")}>
                <Item label={t("settings.updateChannel")}>
                  <Select
                    ariaLabel={tf("settings.selectAriaLabel", [
                      t("settings.updateChannel"),
                      t(CHANNEL_LABEL_KEYS[settings.updateChannel] ?? "settings.channelStable"),
                    ])}
                    value={t(CHANNEL_LABEL_KEYS[settings.updateChannel] ?? "settings.channelStable")}
                  />
                </Item>
                <Item label={t("settings.language")}>
                  <LanguageSelect
                    ariaLabel={t("settings.languageAriaLabel")}
                    value={lang}
                    options={LANGUAGE_OPTIONS}
                    onChange={setLang}
                  />
                </Item>
                <Item label={t("settings.logRetention")}>
                  <Select
                    ariaLabel={tf("settings.selectAriaLabel", [
                      t("settings.logRetention"),
                      t(RETENTION_LABEL_KEYS[settings.logRetention] ?? "settings.retention30"),
                    ])}
                    value={t(RETENTION_LABEL_KEYS[settings.logRetention] ?? "settings.retention30")}
                  />
                </Item>
              </Section>

              <Section label={t("settings.secDiagnostics")}>
                <Item label={t("settings.structuredLogs")}>
                  <Toggle
                    checked={settings.structuredLogs}
                    label={t("settings.structuredLogs")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("structuredLogs")}
                  />
                </Item>
                <Item label={t("settings.anonymousStats")}>
                  <Toggle
                    checked={settings.anonymousUsage}
                    label={t("settings.anonymousStats")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("anonymousUsage")}
                  />
                </Item>
                <Item label={t("settings.crashReporting")}>
                  <Toggle
                    checked={settings.crashReporting}
                    label={t("settings.crashReporting")}
                    onLabel={t("settings.on")}
                    offLabel={t("settings.off")}
                    onChange={() => toggleSetting("crashReporting")}
                  />
                </Item>
              </Section>
            </>
          ) : active === "updates" ? (
            <Section label={t("settings.secCodexUpdates")}>
              <Item label={t("settings.checkCodexUpdatesOnStart")}>
                <Toggle
                  checked={settings.checkCodexUpdatesOnStart}
                  label={t("settings.checkCodexUpdatesOnStart")}
                  onLabel={t("settings.on")}
                  offLabel={t("settings.off")}
                  onChange={() => toggleSetting("checkCodexUpdatesOnStart")}
                />
              </Item>
              <Item label={t("settings.codexUpdateSource")}>
                <SegmentedSetting
                  label={t("settings.codexUpdateSource")}
                  value={settings.codexUpdateSource}
                  options={[
                    { value: "auto", label: t("codex.sourceAuto") },
                    { value: "mirror", label: t("codex.sourceMirror") },
                  ]}
                  onChange={(value) => setSettings((current) => ({ ...current, codexUpdateSource: value }))}
                />
              </Item>
              <Item label={t("settings.codexInstallMode")}>
                <SegmentedSetting
                  label={t("settings.codexInstallMode")}
                  value={settings.codexInstallMode}
                  options={[
                    { value: "standard", label: t("codex.modeStandard") },
                    { value: "portable", label: t("codex.modePortable") },
                  ]}
                  onChange={(value) => setSettings((current) => ({ ...current, codexInstallMode: value }))}
                />
              </Item>
              <div style={{ padding: "14px 0 2px", maxWidth: 620 }}>
                <p style={{ margin: 0, fontSize: 12, lineHeight: 1.6, color: color.muted }}>
                  {t("settings.codexInstallModeHelp")}
                </p>
              </div>
            </Section>
          ) : active === "advanced" ? (
            <>
              <MigrationPanel />
              <PortablePanel mode="cleanup" />
            </>
          ) : active === "about" ? (
            <PortablePanel mode="limits" />
          ) : (
            <Section label={t("settings.secStatus")}>
              <div style={{ height: size.settingsItemRow, display: "flex", alignItems: "center" }}>
                <span style={{ fontSize: 13, color: color.muted }}>
                  {t("settings.categoryNotConfigured")}
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
          {saved && <span style={{ fontSize: 12, color: color.secondary }}>{t("settings.saved")}</span>}
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
              color: color.dangerText,
              fontFamily: "inherit",
              cursor: busy ? "wait" : "pointer",
              opacity: busy ? 0.7 : 1,
            }}
          >
            {t("settings.reset")}
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
            {t("settings.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
