// Chimera++ 2.0 — Appearance feature.
// Layout is a 1:1 implementation of the Pencil design frame `Appearance`.
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
// Note: the skeleton preview bars in the mock spec at #1C1C1C have no exact
// token; color.ink2 (#181818) is the nearest defined surface tone.
import { useState, useEffect } from "react";
import { color, type, size, radius, hairline, ruleOpacity } from "../../design/tokens.ts";

interface Skin {
  id: string;
  name: string;
  subtitle: string;
  applied?: boolean;
}

const DEFAULT_SKINS: Skin[] = [
  { id: "default", name: "Default", subtitle: "Official appearance, no modifications", applied: true },
  { id: "terminal", name: "Terminal", subtitle: "Monospace, high density" },
  { id: "minimal", name: "Minimal", subtitle: "Reduced chrome" },
  { id: "high-contrast", name: "High Contrast", subtitle: "WCAG AAA contrast" },
];

const SAFETY_ROWS: [string, string][] = [
  ["app.asar", "untouched"],
  ["Official files", "untouched"],
  ["CDP", "loopback only"],
  ["JavaScript", "not allowed"],
  ["Remote URLs", "blocked"],
];

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

export function AppearanceFeature() {
  const [skins, setSkins] = useState<Skin[]>(DEFAULT_SKINS);
  const [selectedId, setSelectedId] = useState<string>(
    DEFAULT_SKINS.find(s => s.applied)?.id ?? DEFAULT_SKINS[0].id
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
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

  const selectedSkin = skins.find(s => s.id === selectedId) ?? skins[0];

  async function handleApply() {
    setBusy(true); setError(null);
    try {
      await invoke("apply_skin", { id: selectedSkin.id });
      setSkins(list => list.map(s => ({ ...s, applied: s.id === selectedSkin.id })));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to apply skin.");
    } finally {
      setBusy(false);
    }
  }

  async function handleTry() {
    setBusy(true); setError(null);
    try {
      await invoke("try_skin", { id: selectedSkin.id });
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to preview skin.");
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
      setError(err instanceof Error ? err.message : "Failed to restore default skin.");
    } finally {
      setBusy(false);
    }
  }

  const miniTabs = ["Home", "Providers", "Codex", "Appearance"];

  return (
    <div style={{ display: "flex", height: "100%" }}>
      {/* ── Skin list ── */}
      <div
        role="tablist"
        aria-label="Installed skins"
        style={{
          width: size.skinList, background: color.ink1, borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex", flexDirection: "column",
        }}
      >
        <div style={{
          height: size.panelHead, padding: "0 20px", borderBottom: `${hairline}px solid ${color.rule}`,
          display: "flex", alignItems: "center",
        }}>
          <span style={{ ...type.uiStrong, color: color.secondary }}>Skins</span>
        </div>

        <div style={{ padding: "14px 20px 6px 20px" }}>
          <span style={{ ...type.sectionLabel, color: color.dim }}>INSTALLED</span>
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
                <span style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{skin.name}</span>
                <span style={{ fontSize: 11, color: color.dim }}>{skin.subtitle}</span>
              </div>
              {skin.applied && (
                <span style={{
                  borderRadius: radius.xs, background: color.accentDim, padding: "3px 8px",
                  border: `${hairline}px solid ${color.rule}`, fontSize: 10, fontWeight: 600,
                  letterSpacing: 1.5, color: color.accent,
                }}>
                  APPLIED
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
      <div role="main" aria-label="Skin detail" style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <div style={{
          height: size.panelHead, padding: "0 32px", borderBottom: `${hairline}px solid ${color.rule}`,
          display: "flex", alignItems: "center", gap: 16,
        }}>
          <span style={{ ...type.skinTitle, color: color.primary }}>{selectedSkin.name}</span>
          <span style={{ fontSize: 13, color: color.muted }}>
            {selectedSkin.applied ? "No modifications to official app files" : selectedSkin.subtitle}
          </span>
          <div style={{ flex: 1 }} />
          {selectedSkin.applied && (
            <span style={{ fontSize: 12, color: color.green }}>✓ System default active</span>
          )}
        </div>

        <div style={{ display: "flex", flex: 1 }}>
          <div style={{ flex: 1, padding: "28px 32px", display: "flex", flexDirection: "column", gap: 20 }}>
            <span style={{ ...type.sectionLabel, color: color.dim }}>PREVIEW</span>
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
                <span style={{ fontSize: 9, fontWeight: 600, color: color.primary }}>Chimera++</span>
                <div style={{ width: 1, height: 12, background: color.rule }} />
                {miniTabs.map((tab, i) => (
                  <span key={tab} style={{ fontSize: 8, color: i === 0 ? color.primary : color.muted }}>
                    {tab}
                  </span>
                ))}
              </div>

              <div style={{ padding: "18px 20px", display: "flex", flexDirection: "column" }}>
                <span style={{ fontSize: 7, fontWeight: 600, letterSpacing: 1.5, color: color.muted }}>
                  ACTIVE PROVIDER
                </span>
                <span style={{ fontSize: 26, fontWeight: 700, color: color.primary, lineHeight: 0.95 }}>
                  ChimeraHub
                </span>
                <span style={{ fontSize: 9, color: color.muted }}>→ Codex 26.721 · running</span>
              </div>

              <div style={{ height: 1, background: color.rule }} />

              <div style={{ flex: 1, display: "flex" }}>
                {["PROVIDER", "RUNTIME", "UPDATES"].map((title, i) => (
                  <div
                    key={title}
                    style={{
                      flex: 1, padding: "10px 12px",
                      borderRight: i < 2 ? `${hairline}px solid ${color.rule}` : "none",
                    }}
                  >
                    <span style={{ fontSize: 7, fontWeight: 600, letterSpacing: 1.5, color: color.dim }}>
                      {title}
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
                Apply skin
              </button>
              <button
                onClick={handleTry}
                disabled={busy}
                aria-label="Try without saving"
                style={{
                  background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm, padding: "9px 18px", fontSize: 13, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                Try without saving
              </button>
              <button
                onClick={handleRestore}
                disabled={busy}
                aria-label="Restore default"
                style={{
                  background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
                  borderRadius: radius.sm, padding: "9px 18px", fontSize: 13, fontFamily: "inherit",
                  cursor: busy ? "wait" : "pointer", opacity: busy ? 0.7 : 1,
                }}
              >
                Restore default
              </button>
            </div>

            <div role="alert" aria-live="polite" style={{ minHeight: 16 }}>
              {error && <p style={{ fontSize: 12, color: color.danger, margin: 0 }}>{error}</p>}
            </div>
          </div>

          <div style={{ width: 1, background: color.rule, alignSelf: "stretch" }} />

          <div
            aria-label="Safety information"
            style={{ width: size.skinMeta, padding: "28px 20px", display: "flex", flexDirection: "column" }}
          >
            <span style={{ ...type.sectionLabel, color: color.dim }}>SAFETY</span>
            <div style={{ height: 1, background: color.rule, margin: "8px 0" }} />
            {SAFETY_ROWS.map(([label, value], i) => (
              <div key={label}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, minHeight: 34 }}>
                  <div style={{
                    width: 5, height: 5, borderRadius: "50%",
                    background: color.green,
                  }} />
                  <span style={{ fontSize: 11, color: color.secondary }}>{label}</span>
                  <div style={{ flex: 1 }} />
                  <span style={{ fontSize: 10, color: color.muted, textAlign: "right" }}>{value}</span>
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

