// Chimera++ 2.0 — Providers feature page
// G12: No direct file I/O. State from hooks; actions via Tauri invoke.
// Layout: 1:1 implementation of the Pencil `Providers` screen spec.
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useEffect, useState, type CSSProperties } from "react";
import type {
  ObservedProvider, ProviderEntry, ProviderHealth, ProviderListState,
} from "./lib/providerState.ts";
import {
  createInitialState, addProvider, switchProvider,
  deleteProvider, setHealth, selectActive, hydrateProviderView,
} from "./lib/providerState.ts";
import {
  validateCustomProviderInput, validateChimeraHubKey,
} from "./lib/providerForm.ts";
import { color, type, size, radius, hairline, indicator, ruleOpacity } from "../../design/tokens.ts";
import { useI18n, type TranslationKey } from "../../i18n/index.tsx";

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" &&
  (window as { __TAURI_INTERNALS__?: { invoke?: unknown } }).__TAURI_INTERNALS__?.invoke
    ? (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } })
        .__TAURI_INTERNALS__.invoke
    : async () => undefined;

/** PDot colour: healthy → green, degraded/unknown → amber, failed → danger. */
function dotColor(health: ProviderHealth): string {
  switch (health) {
    case "healthy": return color.green;
    case "unknown":
    case "incompatible": return color.amber;
    case "auth_failed":
    case "unreachable": return color.danger;
  }
}

/** Health label colour used in the detail spec-sheet row. */
const HEALTH_TEXT_COLOR: Record<ProviderHealth, string> = {
  unknown: color.muted,
  healthy: color.greenText,
  auth_failed: color.dangerText,
  incompatible: color.amberText,
  unreachable: color.dangerText,
};

export function ProvidersFeature() {
  const { t, tf } = useI18n();
  const [state, setState] = useState<ProviderListState>(createInitialState());
  const [showAdd, setShowAdd] = useState(false);
  const [addKind, setAddKind] = useState<"chimera_hub" | "custom">("custom");
  const [urlInput, setUrlInput] = useState("");
  const [keyInput, setKeyInput] = useState("");
  const [formErrors, setFormErrors] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [observedProvider, setObservedProvider] = useState<ObservedProvider | null>(null);
  const active = selectActive(state);

  // Hydrate the list from the provider database on every visit. Keeping the
  // initial state in memory only made a restart look like all providers had
  // disappeared and made the first click on "switch" a no-op.
  useEffect(() => {
    let cancelled = false;
    void Promise.all([invoke("list_providers"), invoke("get_system_status")]).then(([raw, statusRaw]) => {
      if (cancelled || !Array.isArray(raw) || !statusRaw || typeof statusRaw !== "object") return;
      const providers: ProviderEntry[] = raw.map((row, index) => {
        const item = row as {
          id?: string; displayName?: string; kind?: string; baseUrl?: string;
          health?: string; selectedModel?: string | null;
        };
        return {
          id: item.id ?? `provider-${index}`,
          displayName: item.displayName ?? "Provider",
          kind: item.kind === "chimerahub" || item.kind === "chimera_hub" ? "chimera_hub" : "custom",
          baseUrl: item.baseUrl ?? "",
          protocol: "responses",
          secretRef: null,
          selectedModel: item.selectedModel ?? null,
          health: (item.health as ProviderHealth) ?? "unknown",
          sortOrder: index,
        };
      });
      const status = statusRaw as {
        activeProviderId?: string | null;
        officialMode?: boolean;
        providerName?: string | null;
        providerUrl?: string | null;
      };
      const hydrated = hydrateProviderView(providers, {
        activeProviderId: status.activeProviderId ?? null,
        officialMode: status.officialMode ?? true,
        providerName: status.providerName ?? null,
        providerUrl: status.providerUrl ?? null,
      });
      setState(hydrated.state);
      setObservedProvider(hydrated.observedProvider);
    });
    return () => { cancelled = true; };
  }, []);

  // Spec 7.1: a provider is verified BEFORE it is activated. The client-side
  // checks below are only a fast pre-filter; the authoritative step is the
  // backend probe inside add_provider, which stores the key and inserts the row
  // only after the endpoint actually answers. A rejected probe adds nothing.
  async function handleAdd() {
    let errors: string[] = [];
    if (addKind === "chimera_hub") {
      const keyError = validateChimeraHubKey(keyInput);
      if (keyError) errors.push(keyError.message);
      errors = errors.concat(validateCustomProviderInput({ url: urlInput, apiKey: "chimera-key-placeholder" })
        .filter(e => e.field === "url" && e.severity === "error").map(e => e.message));
    } else {
      errors = validateCustomProviderInput({ url: urlInput, apiKey: keyInput })
        .filter(e => e.severity === "error").map(e => e.message);
    }
    if (errors.length > 0) { setFormErrors(errors); return; }

    setBusy(true);
    setFormErrors([]);
    try {
      // The key crosses IPC exactly once, here, and is never stored client-side.
      const dto = await invoke("add_provider", {
        kind: addKind,
        baseUrl: urlInput.trim(),
        apiKey: keyInput,
        devMode: false,
      }) as {
        id: string; displayName: string; kind: string;
        baseUrl: string; health: string; selectedModel: string | null;
      } | undefined;

      if (!dto) {
        setFormErrors([t("providers.addFailed")]);
        return;
      }

      // Mirror the row the backend just committed. secretRef is deliberately
      // absent from the DTO (G4) and is not needed to render the list.
      const entry: ProviderEntry = {
        id: dto.id,
        displayName: dto.displayName,
        kind: dto.kind === "chimerahub" ? "chimera_hub" : "custom",
        baseUrl: dto.baseUrl,
        protocol: "responses",
        secretRef: null,
        selectedModel: dto.selectedModel,
        health: dto.health as ProviderEntry["health"],
        sortOrder: state.providers.length,
      };
      setState(s => addProvider(s, entry));
      setObservedProvider(null);
      setShowAdd(false); setUrlInput(""); setKeyInput("");
    } catch (err: unknown) {
      // The backend message is already actionable and localised-safe.
      setFormErrors([err instanceof Error ? err.message : String(err ?? t("providers.verifyFailed"))]);
    } finally {
      setBusy(false);
    }
  }

  async function handleSwitch(id: string | null) {
    setBusy(true);
    try {
      await invoke("switch_provider", { providerId: id });
      setState(s => switchProvider(s, id));
      setObservedProvider(null);
    }
    catch (err) { console.error("switch failed", err); }
    finally { setBusy(false); }
  }

  async function handleTest(id: string) {
    setBusy(true);
    try {
      const result = await invoke("test_existing_provider", { providerId: id }) as { health?: ProviderHealth } | undefined;
      setState(s => setHealth(s, id, result?.health ?? "unknown"));
    } catch (err) {
      console.error("test failed", err);
      setState(s => setHealth(s, id, "unreachable"));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(id: string) {
    setBusy(true);
    try {
      await invoke("delete_provider", { providerId: id });
      setState(s => deleteProvider(s, id));
    } catch (err) {
      setFormErrors([err instanceof Error ? err.message : t("providers.deleteFailed")]);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ height: "100%", display: "flex" }} role="main">
      {/* The panel is a plain container. Only the rows form the tablist —
          a tablist may contain nothing but tabs, so the header and its
          "+ Add" button must sit outside it (axe: aria-required-children). */}
      <div
        style={{
          width: size.providerList, borderRight: `${hairline}px solid ${color.rule}`,
          display: "flex", flexDirection: "column", background: color.ink1,
        }}
      >
        <div style={{
          height: size.panelHead, display: "flex", alignItems: "center", padding: "0 20px",
          borderBottom: `${hairline}px solid ${color.rule}`,
        }}>
          <span style={{ ...type.uiStrong, color: color.secondary }}>
            {state.providers.length} {state.providers.length !== 1 ? t("providers.count") : t("providers.countSingular")}
          </span>
          <span style={{ flex: 1 }} />
          <button
            onClick={() => { setShowAdd(true); setAddKind("chimera_hub"); setUrlInput("https://api.chimerahub.org/v1"); setFormErrors([]); }}
            aria-label={t("providers.addAriaLabel")}
            style={{
              background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
              borderRadius: radius.sm, padding: "5px 10px", fontSize: 12, fontWeight: 700,
              fontFamily: type.family, cursor: "pointer",
            }}
          >
            + {t("providers.add")}
          </button>
        </div>
        <div
          role="tablist"
          aria-label={t("providers.listAriaLabel")}
          aria-orientation="vertical"
          style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0, overflow: "auto" }}
        >
        <button
          role="tab" aria-selected={state.officialMode} onClick={() => handleSwitch(null)} disabled={busy}
          style={{
            width: "100%", height: size.providerRow, padding: "0 20px", display: "flex",
            flexDirection: "column", justifyContent: "center", gap: 3, textAlign: "left",
            background: state.officialMode ? color.ink2 : color.transparent, border: "none",
            borderLeft: `${indicator.rowEdge}px solid ${state.officialMode ? color.accent : color.transparent}`,
            cursor: busy ? "default" : "pointer", fontFamily: type.family,
          }}
        >
          <div style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{t("home.officialCodex")}</div>
          <div style={{ fontSize: 11, color: color.dim }}>{t("providers.officialSystemMode")}</div>
        </button>
        {observedProvider && (
          <button
            type="button"
            role="tab"
            aria-selected="true"
            aria-disabled="true"
            style={{
              width: "100%", height: size.providerRow, padding: "0 20px", display: "flex",
              flexDirection: "column", justifyContent: "center", gap: 3, textAlign: "left",
              background: color.ink2, border: "none",
              borderLeft: `${indicator.rowEdge}px solid ${color.accent}`,
              cursor: "default", fontFamily: type.family,
            }}
          >
            <div style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{observedProvider.displayName}</div>
            <div style={{ fontSize: 11, color: color.dim }}>{t("providers.detectedFromCodex")}</div>
          </button>
        )}
        {state.providers.map(p => (
          <button
            key={p.id} role="tab" aria-selected={state.activeId === p.id} onClick={() => handleSwitch(p.id)} disabled={busy}
            style={{
              width: "100%", height: size.providerRow, padding: "0 20px", display: "flex", alignItems: "center",
              gap: 12, textAlign: "left", background: state.activeId === p.id ? color.ink2 : color.transparent,
              border: "none",
              borderLeft: `${indicator.rowEdge}px solid ${state.activeId === p.id ? color.accent : color.transparent}`,
              cursor: busy ? "default" : "pointer", fontFamily: type.family,
            }}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: 3, flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>{p.displayName}</div>
              <div style={{
                fontSize: 11, color: color.dim, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
              }}>{p.baseUrl}</div>
            </div>
            <span style={{
              width: size.dot, height: size.dot, borderRadius: "50%", background: dotColor(p.health), flexShrink: 0,
            }} />
          </button>
        ))}
        </div>
      </div>
      <div style={{ flex: 1, overflow: "auto" }}>
        {showAdd ? (
          <AddProviderForm
            kind={addKind} urlValue={urlInput} keyValue={keyInput} errors={formErrors}
            onKindChange={(kind) => { setAddKind(kind); if (kind === "chimera_hub" && !urlInput.trim()) setUrlInput("https://api.chimerahub.org/v1"); }} onUrlChange={setUrlInput} onKeyChange={setKeyInput}
            onSubmit={handleAdd} onCancel={() => { setShowAdd(false); setFormErrors([]); }}
          />
        ) : active ? (
          <ProviderDetail
            provider={active} busy={busy}
            onDelete={handleDelete}
            onTest={handleTest}
          />
        ) : observedProvider ? (
          <ObservedProviderDetail provider={observedProvider} />
        ) : (
          <div style={{ padding: "14px 20px 6px 20px" }}>
            <p style={{ ...type.sectionLabel, color: color.dim, margin: "0 0 8px" }}>{t("providers.officialMode")}</p>
            <p style={{ ...type.body, color: color.muted, margin: 0 }}>
              {t("providers.officialModeDesc")}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function ObservedProviderDetail({ provider }: { provider: ObservedProvider }) {
  const { t } = useI18n();
  return (
    <div style={{ padding: "28px 32px", maxWidth: 620 }}>
      <p style={{ ...type.sectionLabel, color: color.accent, margin: "0 0 8px" }}>
        {t("providers.detectedMode")}
      </p>
      <h2 style={{ ...type.pageTitle, color: color.primary, margin: "0 0 8px" }}>
        {provider.displayName}
      </h2>
      <p style={{ ...type.body, color: color.muted, margin: "0 0 24px" }}>
        {t("providers.detectedModeDesc")}
      </p>
      <div style={{ borderTop: `${hairline}px solid ${color.rule}` }}>
        <div style={{ minHeight: 52, display: "flex", alignItems: "center", gap: 24 }}>
          <span style={{ ...type.caption, color: color.muted, width: 120 }}>{t("providers.fieldBaseUrl")}</span>
          <strong style={{ ...type.captionStrong, color: color.secondary, overflowWrap: "anywhere" }}>
            {provider.baseUrl || t("common.dash")}
          </strong>
        </div>
      </div>
      <p style={{ ...type.caption, color: color.dim, margin: "18px 0 0" }}>
        {t("providers.detectedAddHint")}
      </p>
    </div>
  );
}

function AddProviderForm(props: {
  kind: "chimera_hub" | "custom"; urlValue: string; keyValue: string; errors: string[];
  onKindChange: (k: "chimera_hub" | "custom") => void;
  onUrlChange: (v: string) => void; onKeyChange: (v: string) => void;
  onSubmit: () => void; onCancel: () => void;
}) {
  const { t } = useI18n();
  const inp: CSSProperties = {
    width: "100%", background: color.ink2, border: `${hairline}px solid ${color.rule}`,
    borderRadius: radius.sm, color: color.primary, padding: "8px 12px", fontSize: 13,
    fontFamily: type.family, boxSizing: "border-box",
  };
  return (
    <form
      role="form" aria-label={t("providers.addAriaLabel")} onSubmit={e => { e.preventDefault(); props.onSubmit(); }}
      style={{ padding: 32, maxWidth: 480 }}
    >
      <p style={{ ...type.sectionLabel, color: color.muted, margin: "0 0 20px" }}>{t("providers.addSectionLabel")}</p>
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-kind" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
          {t("providers.fieldType")}
        </label>
        <select
          id="add-kind" value={props.kind}
          onChange={e => props.onKindChange(e.target.value as "chimera_hub" | "custom")}
          style={inp}
        >
          <option value="chimera_hub">{t("providers.optionChimeraHub")}</option>
          <option value="custom">{t("providers.optionCustom")}</option>
        </select>
      </div>
      <div style={{ marginBottom: 16 }}>
          <label htmlFor="add-url" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
            {t("providers.fieldBaseUrl")}
          </label>
          <input
            id="add-url" type="url" value={props.urlValue} onChange={e => props.onUrlChange(e.target.value)}
            placeholder={t("providers.urlPlaceholder")} style={inp}
          />
          {props.kind === "chimera_hub" && <span style={{ display: "block", marginTop: 5, fontSize: 11, color: color.muted }}>{t("providers.optionChimeraHub")}</span>}
      </div>
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-key" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
          {t("providers.fieldApiKey")}
        </label>
        <input
          id="add-key" type="password" value={props.keyValue} onChange={e => props.onKeyChange(e.target.value)}
          placeholder={t("providers.keyPlaceholderExample")} style={inp} autoComplete="off"
        />
      </div>
      {props.errors.length > 0 && (
        <ul role="alert" style={{ color: color.danger, fontSize: 12, paddingLeft: 16, margin: "0 0 16px" }}>
          {props.errors.map((e, i) => <li key={i}>{e}</li>)}
        </ul>
      )}
      <div style={{ display: "flex", gap: 10 }}>
        <button
          type="submit"
          style={{
            background: color.accent, color: color.ink0, border: "none", borderRadius: radius.sm,
            padding: "9px 20px", fontSize: 13, fontWeight: 700, fontFamily: type.family, cursor: "pointer",
          }}
        >
          {t("providers.submit")}
        </button>
        <button
          type="button" onClick={props.onCancel}
          style={{
            background: color.ink2, color: color.secondary, border: `${hairline}px solid ${color.rule}`,
            borderRadius: radius.sm, padding: "9px 16px", fontSize: 13, fontFamily: type.family, cursor: "pointer",
          }}
        >
          {t("providers.cancel")}
        </button>
      </div>
    </form>
  );
}

/** Maps a provider health value to its translation key. */
const HEALTH_LABEL_KEY: Record<ProviderHealth, TranslationKey> = {
  unknown: "health.unknown",
  healthy: "health.healthy",
  auth_failed: "health.authFailed",
  incompatible: "health.incompatible",
  unreachable: "health.unreachable",
};

function ProviderDetail({ provider, busy, onDelete, onTest }: {
  provider: ProviderEntry; busy: boolean; onDelete: (id: string) => void; onTest: (id: string) => void;
}) {
  const { t, tf } = useI18n();
  const rows: [string, string, string][] = [
    [t("providers.fieldBaseUrl"), provider.baseUrl, color.secondary],
    [t("providers.fieldProtocol"), provider.protocol, color.secondary],
    [t("providers.fieldModel"), provider.selectedModel ?? t("providers.modelAuto"), color.secondary],
    [t("providers.fieldHealth"), t(HEALTH_LABEL_KEY[provider.health]), HEALTH_TEXT_COLOR[provider.health]],
  ];
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <div style={{
        height: size.panelHead, padding: "0 32px", borderBottom: `${hairline}px solid ${color.rule}`,
        display: "flex", alignItems: "center", gap: 16, flexShrink: 0,
      }}>
        <h2 style={{ ...type.detailTitle, color: color.primary, margin: 0 }}>{provider.displayName}</h2>
        <span style={{
          display: "flex", alignItems: "center", gap: 5, borderRadius: radius.pill,
          background: color.accentDim, padding: "4px 10px", border: `${hairline}px solid ${color.rule}`,
          fontSize: 11,
        }}>
          <span style={{ width: 5, height: 5, borderRadius: "50%", background: color.accent }} />
          <span style={{ color: color.accent }}>{t("providers.active")}</span>
        </span>
        <span style={{ flex: 1 }} />
        <button
          onClick={() => onTest(provider.id)} disabled={busy}
          aria-label={tf("providers.testAria", [provider.displayName])}
          style={{
            background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
            borderRadius: radius.sm, padding: "6px 14px", fontSize: 12, fontFamily: type.family,
            cursor: busy ? "default" : "pointer",
          }}
        >
          {t("providers.test")}
        </button>
      </div>
      <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
        <div style={{ width: size.providerDetailLeft, padding: "28px 32px", display: "flex", flexDirection: "column" }}>
          {rows.map(([k, v, vColor], i) => (
            <div key={k} style={{ display: "flex", flexDirection: "column" }}>
              <div style={{ height: size.codexSpecRow, display: "flex", alignItems: "center" }}>
                <span style={{ fontSize: 12, color: color.muted, width: size.codexSpecKey, flexShrink: 0 }}>{k}</span>
                <span style={{ ...type.captionStrong, color: vColor }}>{v}</span>
              </div>
              {i < rows.length - 1 && (
                <div style={{ height: hairline, background: color.rule, opacity: ruleOpacity.spec }} />
              )}
            </div>
          ))}
        </div>
        <div style={{ width: hairline, background: color.rule, alignSelf: "stretch" }} />
        <div style={{ width: size.providerDetailRight, padding: "28px 24px", display: "flex", flexDirection: "column", gap: 8 }}>
          <button
            aria-label={tf("providers.deleteAria", [provider.displayName])}
            onClick={() => { if (window.confirm(tf("providers.confirmDelete", [provider.displayName]))) onDelete(provider.id); }}
            style={{
              background: color.dangerBg, color: color.danger, border: `${hairline}px solid ${color.dangerBorder}`,
              borderRadius: radius.sm, padding: "8px 16px", fontSize: 13, fontFamily: type.family, cursor: "pointer",
            }}
          >
            {t("providers.deleteAriaLabel")}
          </button>
        </div>
      </div>
    </div>
  );
}
