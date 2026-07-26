// Chimera++ 2.0 — Providers feature page
// G12: No direct file I/O. State from hooks; actions via Tauri invoke.
// Layout: 1:1 implementation of the Pencil `Providers` screen spec.
// Every dimension/colour comes from src/design/tokens.ts — no literals here.
import { useState } from "react";
import type { ProviderEntry, ProviderHealth, ProviderListState } from "./lib/providerState.ts";
import {
  createInitialState, addProvider, switchProvider,
  deleteProvider, setHealth, selectActive,
} from "./lib/providerState.ts";
import {
  validateCustomProviderInput, validateChimeraHubKey,
} from "./lib/providerForm.ts";
import { color, type, size, radius, hairline, indicator, ruleOpacity } from "../../design/tokens.ts";

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
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
  healthy: color.green,
  auth_failed: color.danger,
  incompatible: color.amber,
  unreachable: color.danger,
};

export function ProvidersFeature() {
  const [state, setState] = useState<ProviderListState>(createInitialState());
  const [showAdd, setShowAdd] = useState(false);
  const [addKind, setAddKind] = useState<"chimera_hub" | "custom">("custom");
  const [urlInput, setUrlInput] = useState("");
  const [keyInput, setKeyInput] = useState("");
  const [formErrors, setFormErrors] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const active = selectActive(state);

  function handleAdd() {
    let errors: string[] = [];
    if (addKind === "chimera_hub") {
      const err = validateChimeraHubKey(keyInput);
      if (err) errors.push(err.message);
    } else {
      errors = validateCustomProviderInput({ url: urlInput, apiKey: keyInput })
        .filter(e => e.severity === "error").map(e => e.message);
    }
    if (errors.length > 0) { setFormErrors(errors); return; }
    const entry: ProviderEntry = {
      id: crypto.randomUUID(),
      displayName: addKind === "chimera_hub" ? "ChimeraHub"
        : (() => { try { return new URL(urlInput).hostname; } catch { return "Custom"; } })(),
      kind: addKind,
      baseUrl: addKind === "chimera_hub" ? "https://api.chimerahub.io/v1" : urlInput.trim(),
      protocol: "responses", secretRef: `keychain://chimera/${addKind}`,
      selectedModel: null, health: "unknown", sortOrder: state.providers.length,
    };
    setState(s => addProvider(s, entry));
    setShowAdd(false); setUrlInput(""); setKeyInput(""); setFormErrors([]);
  }

  async function handleSwitch(id: string | null) {
    setBusy(true);
    try { await invoke("switch_provider", { id }); setState(s => switchProvider(s, id)); }
    catch (err) { console.error("switch failed", err); }
    finally { setBusy(false); }
  }

  async function handleTest(id: string) {
    setBusy(true);
    try {
      const result = await invoke("test_provider", { id }) as { health?: ProviderHealth } | undefined;
      setState(s => setHealth(s, id, result?.health ?? "unknown"));
    } catch (err) {
      console.error("test failed", err);
      setState(s => setHealth(s, id, "unreachable"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ height: "100%", display: "flex" }} role="main">
      <nav
        aria-label="Provider list"
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
            {state.providers.length} Provider{state.providers.length !== 1 ? "s" : ""}
          </span>
          <span style={{ flex: 1 }} />
          <button
            onClick={() => setShowAdd(true)}
            aria-label="Add provider"
            style={{
              background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
              borderRadius: radius.sm, padding: "5px 10px", fontSize: 12, fontWeight: 700,
              fontFamily: type.family, cursor: "pointer",
            }}
          >
            + Add
          </button>
        </div>
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
          <div style={{ fontSize: 13, fontWeight: 600, color: color.primary }}>Official Codex</div>
          <div style={{ fontSize: 11, color: color.dim }}>System login mode</div>
        </button>
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
      </nav>
      <div style={{ flex: 1, overflow: "auto" }}>
        {showAdd ? (
          <AddProviderForm
            kind={addKind} urlValue={urlInput} keyValue={keyInput} errors={formErrors}
            onKindChange={setAddKind} onUrlChange={setUrlInput} onKeyChange={setKeyInput}
            onSubmit={handleAdd} onCancel={() => { setShowAdd(false); setFormErrors([]); }}
          />
        ) : active ? (
          <ProviderDetail
            provider={active} busy={busy}
            onDelete={id => setState(s => deleteProvider(s, id))}
            onTest={handleTest}
          />
        ) : (
          <div style={{ padding: "14px 20px 6px 20px" }}>
            <p style={{ ...type.sectionLabel, color: color.dim, margin: "0 0 8px" }}>OFFICIAL MODE</p>
            <p style={{ ...type.body, color: color.muted, margin: 0 }}>
              Codex is using your official login. Add a provider to use a custom API endpoint.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function AddProviderForm(props: {
  kind: "chimera_hub" | "custom"; urlValue: string; keyValue: string; errors: string[];
  onKindChange: (k: "chimera_hub" | "custom") => void;
  onUrlChange: (v: string) => void; onKeyChange: (v: string) => void;
  onSubmit: () => void; onCancel: () => void;
}) {
  const inp: React.CSSProperties = {
    width: "100%", background: color.ink2, border: `${hairline}px solid ${color.rule}`,
    borderRadius: radius.sm, color: color.primary, padding: "8px 12px", fontSize: 13,
    fontFamily: type.family, boxSizing: "border-box",
  };
  return (
    <form
      role="form" aria-label="Add provider" onSubmit={e => { e.preventDefault(); props.onSubmit(); }}
      style={{ padding: 32, maxWidth: 480 }}
    >
      <p style={{ ...type.sectionLabel, color: color.muted, margin: "0 0 20px" }}>ADD PROVIDER</p>
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-kind" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
          Provider type
        </label>
        <select
          id="add-kind" value={props.kind}
          onChange={e => props.onKindChange(e.target.value as "chimera_hub" | "custom")}
          style={inp}
        >
          <option value="chimera_hub">ChimeraHub (built-in template)</option>
          <option value="custom">Custom — URL + API Key</option>
        </select>
      </div>
      {props.kind === "custom" && (
        <div style={{ marginBottom: 16 }}>
          <label htmlFor="add-url" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
            Base URL
          </label>
          <input
            id="add-url" type="url" value={props.urlValue} onChange={e => props.onUrlChange(e.target.value)}
            placeholder="https://api.example.com/v1" style={inp}
          />
        </div>
      )}
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-key" style={{ fontSize: 12, color: color.muted, display: "block", marginBottom: 6 }}>
          API Key
        </label>
        <input
          id="add-key" type="password" value={props.keyValue} onChange={e => props.onKeyChange(e.target.value)}
          placeholder="sk-..." style={inp} autoComplete="off"
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
          Add Provider
        </button>
        <button
          type="button" onClick={props.onCancel}
          style={{
            background: color.ink2, color: color.secondary, border: `${hairline}px solid ${color.rule}`,
            borderRadius: radius.sm, padding: "9px 16px", fontSize: 13, fontFamily: type.family, cursor: "pointer",
          }}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}

function ProviderDetail({ provider, busy, onDelete, onTest }: {
  provider: ProviderEntry; busy: boolean; onDelete: (id: string) => void; onTest: (id: string) => void;
}) {
  const rows: [string, string, string][] = [
    ["Base URL", provider.baseUrl, color.secondary],
    ["Protocol", provider.protocol, color.secondary],
    ["Model", provider.selectedModel ?? "Auto", color.secondary],
    ["Health", provider.health.replace(/_/g, " "), HEALTH_TEXT_COLOR[provider.health]],
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
          <span style={{ color: color.accent }}>Active</span>
        </span>
        <span style={{ flex: 1 }} />
        <button
          onClick={() => onTest(provider.id)} disabled={busy}
          aria-label={"Test connection to " + provider.displayName}
          style={{
            background: color.ink3, color: color.primary, border: `${hairline}px solid ${color.rule}`,
            borderRadius: radius.sm, padding: "6px 14px", fontSize: 12, fontFamily: type.family,
            cursor: busy ? "default" : "pointer",
          }}
        >
          Test connection
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
            aria-label={"Delete provider " + provider.displayName}
            onClick={() => { if (window.confirm("Delete " + provider.displayName + "?")) onDelete(provider.id); }}
            style={{
              background: color.dangerBg, color: color.danger, border: `${hairline}px solid ${color.dangerBorder}`,
              borderRadius: radius.sm, padding: "8px 16px", fontSize: 13, fontFamily: type.family, cursor: "pointer",
            }}
          >
            Delete provider
          </button>
        </div>
      </div>
    </div>
  );
}
