import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import { color, type, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";

interface CatalogSkin {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  appearance?: string;
  license?: string;
  category?: string;
  codexVerified?: string;
  bytes: number;
  pack: string;
  preview: string;
  installed: boolean;
  applied: boolean;
}

const SKINS_BASE = "https://skins.agentsmirror.com";
const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

const SAFETY_ROWS: { labelKey: TranslationKey; valueKey: TranslationKey }[] = [
  { labelKey: "appearance.rowAppAsar", valueKey: "appearance.valUntouched" },
  { labelKey: "appearance.rowOfficialFiles", valueKey: "appearance.valUntouched" },
  { labelKey: "appearance.rowCdp", valueKey: "appearance.valLoopbackOnly" },
  { labelKey: "appearance.rowJavaScript", valueKey: "appearance.valNotAllowed" },
  { labelKey: "appearance.rowRemoteUrls", valueKey: "appearance.valBlocked" },
];

function previewUrl(skin: CatalogSkin): string {
  return `${SKINS_BASE}/${skin.preview}`;
}

function formatSize(bytes: number): string {
  return `${Math.max(1, Math.round(bytes / 1024 / 1024))} MB`;
}

export function AppearanceFeature() {
  const { t } = useI18n();
  const [skins, setSkins] = useState<CatalogSkin[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | "installed">("all");
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setError(null);
    try {
      const result = await invoke("list_skin_catalog");
      const next = Array.isArray(result) ? result as CatalogSkin[] : [];
      setSkins(next);
      setSelectedId((current) => current && next.some((skin) => skin.id === current)
        ? current
        : next.find((skin) => skin.applied)?.id ?? next[0]?.id ?? null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errCatalog"));
    }
  }, [t]);

  useEffect(() => { void reload(); }, [reload]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<{ downloaded: number; total: number }>("skin://download-progress", (event) => {
      if (!disposed && event.payload.total > 0) {
        setProgress(Math.min(100, Math.round(event.payload.downloaded / event.payload.total * 100)));
      }
    }).then((stop) => { unlisten = stop; });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const visibleSkins = useMemo(
    () => filter === "installed" ? skins.filter((skin) => skin.installed) : skins,
    [filter, skins],
  );
  const selected = skins.find((skin) => skin.id === selectedId) ?? visibleSkins[0];

  async function run(command: string, args: Record<string, unknown>, success: TranslationKey) {
    setBusy(true);
    setProgress(0);
    setError(null);
    setMessage(null);
    try {
      await invoke(command, args);
      setMessage(t(success));
      await reload();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errApply"));
    } finally {
      setBusy(false);
    }
  }

  async function handleImport() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Codex skin", extensions: ["codexskin"] }],
    });
    if (typeof picked !== "string") return;
    await run("import_skin_package", { path: picked }, "appearance.imported");
  }

  return (
    <div style={{ display: "flex", height: "100%" }}>
      <aside
        aria-label={t("appearance.listAriaLabel")}
        style={{
          width: size.skinList,
          background: color.ink1,
          borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex",
          flexDirection: "column",
          minWidth: 260,
        }}
      >
        <div style={{ height: size.panelHead, padding: "0 20px", borderBottom: `${hairline}px solid ${color.rule}`, display: "flex", alignItems: "center", gap: 12 }}>
          <span style={{ ...type.uiStrong, color: color.secondary }}>{t("appearance.skins")}</span>
          <div style={{ flex: 1 }} />
          <button type="button" onClick={() => void handleImport()} disabled={busy} style={textButtonStyle}>
            {t("appearance.importSkin")}
          </button>
        </div>

        <div role="tablist" aria-label={t("appearance.filterAriaLabel")} style={{ padding: "12px 16px", display: "flex", gap: 3 }}>
          {(["all", "installed"] as const).map((value) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={filter === value}
              onClick={() => setFilter(value)}
              style={{
                flex: 1,
                minHeight: 30,
                border: "none",
                borderRadius: radius.xs,
                background: filter === value ? color.ink3 : color.transparent,
                color: filter === value ? color.primary : color.muted,
                fontFamily: "inherit",
                fontSize: 11,
                cursor: "pointer",
              }}
            >
              {value === "all" ? t("appearance.marketplace") : t("appearance.installed")}
            </button>
          ))}
        </div>

        <div style={{ overflowY: "auto", flex: 1 }}>
          {visibleSkins.map((skin, index) => (
            <div key={skin.id}>
              <button
                type="button"
                onClick={() => setSelectedId(skin.id)}
                aria-pressed={skin.id === selected?.id}
                style={{
                  width: "100%",
                  minHeight: 68,
                  padding: "10px 18px",
                  display: "flex",
                  alignItems: "center",
                  gap: 11,
                  border: "none",
                  borderLeft: `2px solid ${skin.id === selected?.id ? color.accent : color.transparent}`,
                  background: skin.id === selected?.id ? color.ink2 : color.transparent,
                  color: color.primary,
                  textAlign: "left",
                  fontFamily: "inherit",
                  cursor: "pointer",
                }}
              >
                <img src={previewUrl(skin)} alt="" loading="lazy" style={{ width: 52, height: 34, objectFit: "cover", borderRadius: radius.xs, border: `${hairline}px solid ${color.rule}` }} />
                <span style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 0, flex: 1 }}>
                  <span style={{ fontSize: 12, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{skin.name}</span>
                  <span style={{ fontSize: 10, color: skin.installed ? color.greenText : color.dim }}>
                    {skin.installed ? t("appearance.installedBadge") : `${skin.version} · ${formatSize(skin.bytes)}`}
                  </span>
                </span>
              </button>
              {index < visibleSkins.length - 1 && <div style={{ height: 1, background: color.rule, opacity: ruleOpacity.list }} />}
            </div>
          ))}
          {!busy && visibleSkins.length === 0 && <p style={{ padding: "16px 20px", margin: 0, fontSize: 12, color: color.muted }}>{t("appearance.empty")}</p>}
        </div>
      </aside>

      <main aria-label={t("appearance.detailAriaLabel")} style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <header style={{ minHeight: size.panelHead, padding: "12px 32px", borderBottom: `${hairline}px solid ${color.rule}`, display: "flex", alignItems: "center", gap: 14 }}>
          <div style={{ minWidth: 0 }}>
            <h2 style={{ ...type.skinTitle, color: color.primary, margin: 0 }}>{selected?.name ?? t("appearance.skins")}</h2>
            <p style={{ margin: "4px 0 0", fontSize: 11, color: color.muted }}>
              {selected ? `${selected.author} · ${selected.version} · Codex ${selected.codexVerified ?? t("common.dash")}` : t("appearance.loadingCatalog")}
            </p>
          </div>
          <div style={{ flex: 1 }} />
          {selected?.applied && <span style={{ fontSize: 11, color: color.greenText }}>{t("appearance.applied")}</span>}
        </header>

        <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
          <section style={{ flex: 1, padding: "26px 32px", display: "flex", flexDirection: "column", gap: 16, minWidth: 0, overflowY: "auto" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <span style={{ ...type.sectionLabel, color: color.dim }}>{t("appearance.realPreview")}</span>
              <div style={{ flex: 1, height: 1, background: color.rule }} />
              {selected?.category && <span style={{ fontSize: 10, color: color.muted }}>{selected.category}</span>}
            </div>

            <div style={{ aspectRatio: "16 / 10", width: "100%", maxHeight: 520, background: color.ink0, border: `${hairline}px solid ${color.rule}`, borderRadius: radius.md, overflow: "hidden", display: "flex", alignItems: "center", justifyContent: "center" }}>
              {selected
                ? <img src={previewUrl(selected)} alt={selected.name} style={{ display: "block", width: "100%", height: "100%", objectFit: "contain" }} />
                : <span style={{ fontSize: 12, color: color.muted }}>{t("appearance.loadingCatalog")}</span>}
            </div>

            <p style={{ margin: 0, minHeight: 38, maxWidth: 820, fontSize: 12, lineHeight: 1.6, color: color.secondary }}>
              {selected?.description}
            </p>

            {busy && (
              <div aria-live="polite" style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <div style={{ height: 4, flex: 1, borderRadius: radius.xs, background: color.ink3, overflow: "hidden" }}>
                  <div style={{ width: `${progress}%`, height: "100%", background: color.accent }} />
                </div>
                <span style={{ width: 32, fontSize: 10, color: color.muted }}>{progress}%</span>
              </div>
            )}

            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {selected && !selected.installed && (
                <button type="button" disabled={busy} onClick={() => void run("install_catalog_skin", { skinId: selected.id }, "appearance.installedSuccess")} style={primaryButtonStyle}>
                  {t("appearance.install")}
                </button>
              )}
              {selected?.installed && (
                <>
                  <button type="button" disabled={busy} onClick={() => void run("apply_skin_package", { skinId: selected.id }, "appearance.appliedSuccess")} style={primaryButtonStyle}>
                    {t("appearance.apply")}
                  </button>
                  <button type="button" disabled={busy} onClick={() => void run("try_skin_package", { skinId: selected.id }, "appearance.previewStarted")} style={secondaryButtonStyle}>
                    {t("appearance.tryIt")}
                  </button>
                </>
              )}
              <button type="button" disabled={busy} onClick={() => void run("restore_skin_package", {}, "appearance.restoredSuccess")} style={secondaryButtonStyle}>
                {t("appearance.restoreDefault")}
              </button>
            </div>

            <div role="status" aria-live="polite" style={{ minHeight: 18 }}>
              {error && <p style={{ margin: 0, fontSize: 12, color: color.danger }}>{error}</p>}
              {!error && message && <p style={{ margin: 0, fontSize: 12, color: color.greenText }}>{message}</p>}
            </div>
          </section>

          <div style={{ width: 1, background: color.rule }} />
          <aside aria-label={t("appearance.safetyAriaLabel")} style={{ width: size.skinMeta, padding: "26px 20px", overflowY: "auto" }}>
            <span style={{ ...type.sectionLabel, color: color.dim }}>{t("appearance.safety")}</span>
            <div style={{ height: 1, background: color.rule, margin: "9px 0" }} />
            {SAFETY_ROWS.map((row, index) => (
              <div key={row.labelKey}>
                <div style={{ minHeight: 38, display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ width: 5, height: 5, borderRadius: "50%", background: color.green, flexShrink: 0 }} />
                  <span style={{ fontSize: 10, color: color.secondary }}>{t(row.labelKey)}</span>
                  <span style={{ flex: 1 }} />
                  <span style={{ fontSize: 9, color: color.muted, textAlign: "right" }}>{t(row.valueKey)}</span>
                </div>
                {index < SAFETY_ROWS.length - 1 && <div style={{ height: 1, background: color.rule, opacity: ruleOpacity.spec }} />}
              </div>
            ))}
          </aside>
        </div>
      </main>
    </div>
  );
}

const textButtonStyle = {
  border: "none",
  background: "transparent",
  color: color.secondary,
  fontFamily: "inherit",
  fontSize: 11,
  cursor: "pointer",
} as const;

const primaryButtonStyle = {
  minHeight: 34,
  padding: "0 18px",
  border: "none",
  borderRadius: radius.sm,
  background: color.accent,
  color: color.ink0,
  fontFamily: "inherit",
  fontSize: 12,
  fontWeight: 700,
  cursor: "pointer",
} as const;

const secondaryButtonStyle = {
  minHeight: 34,
  padding: "0 16px",
  border: `${hairline}px solid ${color.rule}`,
  borderRadius: radius.sm,
  background: color.ink3,
  color: color.primary,
  fontFamily: "inherit",
  fontSize: 12,
  cursor: "pointer",
} as const;
