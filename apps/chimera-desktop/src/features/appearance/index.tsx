// Chimera++ 2.0 — ChatGPT / Codex appearance feature.
// Layout is a 1:1 implementation of the Pencil design frame `Appearance`.
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
// The preview mirrors ChatGPT/Codex appearance surfaces; it is intentionally
// separate from Chimera++'s own shell theme.
import { useCallback, useState, useEffect } from "react";
import { color, type, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";

interface Skin {
  id: string;
  // Built-in skins are localized via i18n key. Module-level constants must
  // hold KEYS, never translated text, so instant language switching works —
  // scripts/verify-i18n.mjs enforces this.
  nameKey?: TranslationKey;
  subtitleKey?: TranslationKey;
  // Skins loaded from disk/backend carry their own display strings — they
  // are not part of the translation dictionary, so these are plain text.
  name?: string;
  /// The backend's SkinDto calls this `description`; a package that no longer
  /// validates puts its error here, so it is shown rather than hidden.
  description?: string;
  applied?: boolean;
}

// Only the built-in default. The three sample skins that used to sit here were
// placeholders with no package behind them: once list_skins became real they
// would have been offered to a user and then failed to apply, which is worse
// than an empty list. Everything else arrives from the backend.
const DEFAULT_SKINS: Skin[] = [
  { id: "default", nameKey: "appearance.defaultName", subtitleKey: "appearance.defaultSubtitle", applied: true },
];

const SAFETY_ROWS: { labelKey: TranslationKey; valueKey: TranslationKey }[] = [
  { labelKey: "appearance.rowAppAsar", valueKey: "appearance.valUntouched" },
  { labelKey: "appearance.rowOfficialFiles", valueKey: "appearance.valUntouched" },
  { labelKey: "appearance.rowCdp", valueKey: "appearance.valLoopbackOnly" },
  { labelKey: "appearance.rowJavaScript", valueKey: "appearance.valNotAllowed" },
  { labelKey: "appearance.rowRemoteUrls", valueKey: "appearance.valBlocked" },
];

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

export function AppearanceFeature() {
  const { t, tf } = useI18n();
  const [skins, setSkins] = useState<Skin[]>(DEFAULT_SKINS);
  const [selectedId, setSelectedId] = useState<string>(
    DEFAULT_SKINS.find(s => s.applied)?.id ?? DEFAULT_SKINS[0].id
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Resolve a skin's display strings: built-in skins translate via key,
  // skins loaded from disk/backend use their own raw name/subtitle.
  function skinName(skin: Skin): string {
    return skin.nameKey ? t(skin.nameKey) : skin.name ?? "";
  }
  function skinSubtitle(skin: Skin): string {
    return skin.subtitleKey ? t(skin.subtitleKey) : skin.description ?? "";
  }

  const reload = useCallback(() => {
    invoke("list_skins")
      .then(list => {
        if (Array.isArray(list) && list.length > 0) {
          const next = list as Skin[];
          setSkins(next);
          const applied = next.find(s => s.applied);
          if (applied) setSelectedId(applied.id);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => { reload(); }, [reload]);

  /// Pick a .codexskin and import it. Validation happens in the backend before
  /// the file is stored, so a rejected package is never written to disk.
  async function handleImport() {
    setBusy(true); setError(null);
    try {
      const dialog = (globalThis as { __TAURI__?: { dialog?: { open?: (o: unknown) => Promise<unknown> } } }).__TAURI__?.dialog;
      const picked = await dialog?.open?.({
        multiple: false,
        filters: [{ name: "Codex skin", extensions: ["codexskin"] }],
      });
      if (typeof picked !== "string") return;
      await invoke("import_skin", { path: picked });
      reload();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errImport"));
    } finally {
      setBusy(false);
    }
  }

  const selectedSkin = skins.find(s => s.id === selectedId) ?? skins[0];

  async function handleApply() {
    setBusy(true); setError(null);
    try {
      await invoke("apply_skin", { skinId: selectedSkin.id });
      setSkins(list => list.map(s => ({ ...s, applied: s.id === selectedSkin.id })));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errApply"));
    } finally {
      setBusy(false);
    }
  }

  async function handleTry() {
    setBusy(true); setError(null);
    try {
      await invoke("try_skin", { skinId: selectedSkin.id });
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errTry"));
    } finally {
      setBusy(false);
    }
  }

  async function handleRestore() {
    setBusy(true); setError(null);
    try {
      await invoke("restore_default_skin");
      setSkins(list => list.map(s => ({ ...s, applied: s.id === "default" })));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t("appearance.errRestore"));
    } finally {
      setBusy(false);
    }
  }

  const miniTabs: TranslationKey[] = ["nav.home", "nav.providers", "nav.codex", "nav.appearance"];
  const statCols: TranslationKey[] = ["home.colProvider", "home.colRuntime", "home.colUpdates"];

  return (
    <div style={{ display: "flex", height: "100%" }}>
      {/* ── Skin list ── */}
      <div
        role="tablist"
        aria-label={t("appearance.listAriaLabel")}
        style={{
          width: size.skinList, background: color.ink1, borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex", flexDirection: "column",
        }}
      >
        <div style={{
          height: size.panelHead, padding: "0 20px", borderBottom: `${hairline}px solid ${color.rule}`,
          display: "flex", alignItems: "center",
        }}>
          <span style={{ ...type.uiStrong, color: color.secondary }}>
            {t("appearance.skins")}
          </span>
        </div>

        <div style={{ padding: "14px 20px 6px 20px" }}>
          <span style={{ ...type.sectionLabel, color: color.dim }}>
            {t("appearance.installed")}
          </span>
        </div>

        {skins.map((skin, i) => (
          <div key={skin.id}>
            <div
              role="tab"
              aria-selected={skin.id === selectedId}
              tabIndex={0}
              onClick={() => setSelectedId(skin.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setSelectedId(skin.id); }
              }}
              style={{
                height: size.skinRow, padding: "0 20px", display: "flex", alignItems: "center", gap: 12,
                background: skin.applied ? color.ink2 : color.transparent,
                borderLeft: `2px solid ${skin.applied ? color.accent : color.transparent}`,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 3, flex: 1 }}>
                <span style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{skinName(skin)}</span>
                <span style={{ fontSize: 11, color: color.dim }}>{skinSubtitle(skin)}</span>
              </div>
              {skin.applied && (
                <span style={{
                  borderRadius: radius.xs, background: color.accentDim, padding: "3px 8px",
                  border: `${hairline}px solid ${color.rule}`, fontSize: 10, fontWeight: 600,
                  letterSpacing: 1.5, color: color.accent,
                }}>
                  {t("appearance.applied")}
                </span>
              )}
            </div>
            {i < skins.length - 1 && (
              <div style={{ height: 1, background: color.rule, opacity: ruleOpacity.list }} />
            )}
          </div>
        ))}
      </div>

      {/* ── Skin detail ── */}
      <div role="main" aria-label={t("appearance.detailAriaLabel")} style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        <div style={{
          height: size.panelHead, padding: "0 32px", borderBottom: `${hairline}px solid ${color.rule}`,
          display: "flex", alignItems: "center", gap: 16,
        }}>
          <span style={{ ...type.skinTitle, color: color.primary }}>{skinName(selectedSkin)}</span>
          <span style={{ fontSize: 13, color: color.muted }}>
            {selectedSkin.applied ? t("appearance.defaultDesc") : skinSubtitle(selectedSkin)}
          </span>
          <div style={{ flex: 1 }} />
          {selectedSkin.applied && (
            <span style={{ fontSize: 12, color: color.greenText }}>
              {t("appearance.systemDefaultActive")}
            </span>
          )}
        </div>

        <div style={{ display: "flex", flex: 1 }}>
          <div style={{ flex: 1, padding: "28px 32px", display: "flex", flexDirection: "column", gap: 20 }}>
            <span style={{ ...type.sectionLabel, color: color.dim }}>
              {t("appearance.preview")}
            </span>
            <div style={{ height: 1, background: color.rule }} />

            <div
              aria-hidden="true"
              style={{
                flex: 1, background: color.ink0, border: `${hairline}px solid ${color.rule}`,
                borderRadius: radius.md, display: "flex", flexDirection: "column",
                overflow: "hidden", minHeight: 390,
              }}
            >
              <div style={{
                height: 28, background: color.ink1, borderBottom: `${hairline}px solid ${color.rule}`,
                display: "flex", alignItems: "center", padding: "0 10px", gap: 6,
              }}>
                <div style={{ width: 10, height: 10, borderRadius: radius.xs, background: color.accent }} />
                <span style={{ fontSize: 9, fontWeight: 600, color: color.primary }}>
                  Chimera++
                </span>
                <div style={{ width: 1, height: 12, background: color.rule }} />
                {miniTabs.map((key, i) => (
                  <span key={key} style={{ fontSize: 8, color: i === 0 ? color.primary : color.muted }}>
                    {t(key)}
                  </span>
                ))}
              </div>

              <div style={{ padding: "18px 20px", display: "flex", flexDirection: "column" }}>
                <span style={{ fontSize: 7, fontWeight: 600, letterSpacing: 1.5, color: color.muted }}>
                  {t("home.eyebrow")}
                </span>
                <span style={{ fontSize: 26, fontWeight: 700, color: color.primary, lineHeight: 0.95 }}>
                  ChimeraHub
                </span>
                <span style={{ fontSize: 9, color: color.muted }}>
                  {tf("appearance.previewStatusLine", ["26.721"])}
                </span>
              </div>

              <div style={{ height: 1, background: color.rule }} />

              <div style={{ flex: 1, display: "flex" }}>
                {statCols.map((key, i) => (
                  <div
                    key={key}
                    style={{
                      flex: 1, padding: "10px 12px",
                      borderRight: i < 2 ? `${hairline}px solid ${color.rule}` : "none",
                    }}
                  >
                    <span style={{ fontSize: 7, fontWeight: 600, letterSpacing: 1.5, color: color.dim }}>
                      {t(key)}
                    </span>
                    {[0, 1, 2].map(row => (
                      <div key={row} style={{
                        height: 6, background: color.ink2, borderRadius: radius.xs, marginTop: 5,
                      }} />
                    ))}
                  </div>
                ))}
              </div>
            </div>

            <div style={{ display: "flex", gap: 10 }}>
              <button
                onClick={handleApply}
                disabled={busy}
                style={{
                  background: color.accent, color: color.ink0, border: "none", borderRadius: radius.sm,
                  padding: "9px 20px", fontSize: 13, fontWeight: 700, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                {t("appearance.apply")}
              </button>
              <button
                onClick={handleTry}
                disabled={busy}
                aria-label={t("appearance.tryIt")}
                style={{
                  background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm, padding: "9px 18px", fontSize: 13, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                {t("appearance.tryIt")}
              </button>
              <button
                onClick={handleRestore}
                disabled={busy}
                aria-label={t("appearance.restoreDefault")}
                style={{
                  background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm, padding: "9px 18px", fontSize: 13, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                {t("appearance.restoreDefault")}
              </button>
              {/* Without this there is no way to get a skin into the app at
                  all — the list can only ever show the built-in default. */}
              <button
                onClick={handleImport}
                disabled={busy}
                aria-label={t("appearance.importSkin")}
                style={{
                  background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm, padding: "9px 18px", fontSize: 13, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                {t("appearance.importSkin")}
              </button>
            </div>

            <div role="alert" aria-live="polite" style={{ minHeight: 16 }}>
              {error && <p style={{ fontSize: 12, color: color.danger, margin: 0 }}>{error}</p>}
            </div>
          </div>

          <div style={{ width: 1, background: color.rule, alignSelf: "stretch" }} />

          <div
            aria-label={t("appearance.safetyAriaLabel")}
            style={{ width: size.skinMeta, padding: "28px 20px", display: "flex", flexDirection: "column" }}
          >
            <span style={{ ...type.sectionLabel, color: color.dim }}>
              {t("appearance.safety")}
            </span>
            <div style={{ height: 1, background: color.rule, margin: "8px 0" }} />
            {SAFETY_ROWS.map((row, i) => (
              <div key={row.labelKey}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, minHeight: 34 }}>
                  <div style={{
                    width: 5, height: 5, borderRadius: "50%",
                    background: color.green,
                  }} />
                  <span style={{ fontSize: 11, color: color.secondary }}>{t(row.labelKey)}</span>
                  <div style={{ flex: 1 }} />
                  <span style={{ fontSize: 10, color: color.muted, textAlign: "right" }}>{t(row.valueKey)}</span>
                </div>
                {i < SAFETY_ROWS.length - 1 && (
                  <div style={{ height: 1, background: color.rule, opacity: ruleOpacity.spec }} />
                )}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
