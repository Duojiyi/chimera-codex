// Chimera++ 2.0 — Providers feature page
// G12: No direct file I/O. State from hooks; actions via Tauri invoke.
import { useState } from "react";
import type { ProviderEntry, ProviderListState } from "./lib/providerState.ts";
import {
  createInitialState, addProvider, switchProvider,
  deleteProvider, selectActive,
} from "./lib/providerState.ts";
import {
  validateCustomProviderInput, validateChimeraHubKey,
} from "./lib/providerForm.ts";

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__
    ? (window as any).__TAURI_INTERNALS__.invoke
    : async () => undefined;

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

  return (
    <div style={{ height: "100%", display: "flex" }} role="main">
      <nav aria-label="Provider list" style={{ width: 300, borderRight: "1px solid #282828", display: "flex", flexDirection: "column", background: "#111111" }}>
        <div style={{ height: 52, display: "flex", alignItems: "center", padding: "0 20px", borderBottom: "1px solid #282828", justifyContent: "space-between" }}>
          <span style={{ fontSize: 13, fontWeight: 600, color: "#999" }}>{state.providers.length} Provider{state.providers.length !== 1 ? "s" : ""}</span>
          <button onClick={() => setShowAdd(true)} aria-label="Add provider" style={{ background: "#FF4D3D", color: "#0C0C0C", border: "none", borderRadius: 3, padding: "5px 10px", fontSize: 12, fontWeight: 700, cursor: "pointer" }}>+ Add</button>
        </div>
        <button role="tab" aria-selected={state.officialMode} onClick={() => handleSwitch(null)} disabled={busy} style={{ width: "100%", padding: "12px 20px", textAlign: "left", background: state.officialMode ? "#1C1C1C" : "transparent", border: "none", borderLeft: state.officialMode ? "2px solid #FF4D3D" : "2px solid transparent", cursor: "pointer", color: state.officialMode ? "#EBEBEB" : "#5E5E5E" }}>
          <div style={{ fontSize: 13, fontWeight: state.officialMode ? 600 : 400 }}>Official Codex</div>
          <div style={{ fontSize: 11, color: "#3A3A3A", marginTop: 2 }}>System login mode</div>
        </button>
        {state.providers.map(p => (
          <button key={p.id} role="tab" aria-selected={state.activeId === p.id} onClick={() => handleSwitch(p.id)} disabled={busy} style={{ width: "100%", padding: "12px 20px", textAlign: "left", background: state.activeId === p.id ? "#1C1C1C" : "transparent", border: "none", borderLeft: state.activeId === p.id ? "2px solid #FF4D3D" : "2px solid transparent", cursor: "pointer", color: state.activeId === p.id ? "#EBEBEB" : "#5E5E5E" }}>
            <div style={{ fontSize: 13, fontWeight: state.activeId === p.id ? 600 : 400 }}>{p.displayName}</div>
            <div style={{ fontSize: 11, color: "#3A3A3A", marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{p.baseUrl}</div>
          </button>
        ))}
      </nav>
      <div style={{ flex: 1, overflow: "auto" }}>
        {showAdd ? (
          <AddProviderForm kind={addKind} urlValue={urlInput} keyValue={keyInput} errors={formErrors} onKindChange={setAddKind} onUrlChange={setUrlInput} onKeyChange={setKeyInput} onSubmit={handleAdd} onCancel={() => { setShowAdd(false); setFormErrors([]); }} />
        ) : active ? (
          <ProviderDetail provider={active} onDelete={id => setState(s => deleteProvider(s, id))} />
        ) : (
          <div style={{ padding: 40 }}>
            <p style={{ fontSize: 10, fontWeight: 600, letterSpacing: 1.5, color: "#3A3A3A" }}>OFFICIAL MODE</p>
            <p style={{ fontSize: 14, color: "#5E5E5E" }}>Codex is using your official login. Add a provider to use a custom API endpoint.</p>
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
  const inp: React.CSSProperties = { width: "100%", background: "#1C1C1C", border: "1px solid #282828", borderRadius: 3, color: "#EBEBEB", padding: "8px 12px", fontSize: 13, boxSizing: "border-box" };
  return (
    <form role="form" aria-label="Add provider" onSubmit={e => { e.preventDefault(); props.onSubmit(); }} style={{ padding: 32, maxWidth: 480 }}>
      <p style={{ fontSize: 10, fontWeight: 600, letterSpacing: 1.5, color: "#5E5E5E", margin: "0 0 20px" }}>ADD PROVIDER</p>
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-kind" style={{ fontSize: 12, color: "#5E5E5E", display: "block", marginBottom: 6 }}>Provider type</label>
        <select id="add-kind" value={props.kind} onChange={e => props.onKindChange(e.target.value as "chimera_hub" | "custom")} style={inp}>
          <option value="chimera_hub">ChimeraHub (built-in template)</option>
          <option value="custom">Custom — URL + API Key</option>
        </select>
      </div>
      {props.kind === "custom" && (
        <div style={{ marginBottom: 16 }}>
          <label htmlFor="add-url" style={{ fontSize: 12, color: "#5E5E5E", display: "block", marginBottom: 6 }}>Base URL</label>
          <input id="add-url" type="url" value={props.urlValue} onChange={e => props.onUrlChange(e.target.value)} placeholder="https://api.example.com/v1" style={inp} />
        </div>
      )}
      <div style={{ marginBottom: 16 }}>
        <label htmlFor="add-key" style={{ fontSize: 12, color: "#5E5E5E", display: "block", marginBottom: 6 }}>API Key</label>
        <input id="add-key" type="password" value={props.keyValue} onChange={e => props.onKeyChange(e.target.value)} placeholder="sk-..." style={inp} autoComplete="off" />
      </div>
      {props.errors.length > 0 && (
        <ul role="alert" style={{ color: "#FF453A", fontSize: 12, paddingLeft: 16, margin: "0 0 16px" }}>
          {props.errors.map((e, i) => <li key={i}>{e}</li>)}
        </ul>
      )}
      <div style={{ display: "flex", gap: 10 }}>
        <button type="submit" style={{ background: "#FF4D3D", color: "#0C0C0C", border: "none", borderRadius: 3, padding: "9px 20px", fontSize: 13, fontWeight: 700, cursor: "pointer" }}>Add Provider</button>
        <button type="button" onClick={props.onCancel} style={{ background: "#1C1C1C", color: "#999", border: "1px solid #282828", borderRadius: 3, padding: "9px 16px", fontSize: 13, cursor: "pointer" }}>Cancel</button>
      </div>
    </form>
  );
}

function ProviderDetail({ provider, onDelete }: { provider: ProviderEntry; onDelete: (id: string) => void }) {
  const hc: Record<string, string> = { unknown: "#5E5E5E", healthy: "#34C759", auth_failed: "#FF453A", incompatible: "#FF9F0A", unreachable: "#FF453A" };
  return (
    <div style={{ padding: 32 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 14, marginBottom: 24 }}>
        <h2 style={{ margin: 0, fontSize: 26, fontWeight: 700, color: "#EBEBEB" }}>{provider.displayName}</h2>
        <span style={{ fontSize: 11, fontWeight: 600, color: hc[provider.health] ?? "#5E5E5E", letterSpacing: 1 }}>
          {provider.health.replace(/_/g, " ").toUpperCase()}
        </span>
      </div>
      <table style={{ borderCollapse: "collapse", width: "100%", maxWidth: 540 }}>
        <tbody>
          {([["Base URL", provider.baseUrl], ["Protocol", provider.protocol], ["Model", provider.selectedModel ?? "Auto"]] as [string, string][]).map(([k, v]) => (
            <tr key={k} style={{ borderBottom: "1px solid #1A1A1A" }}>
              <td style={{ padding: "9px 0", fontSize: 12, color: "#5E5E5E", width: 130 }}>{k}</td>
              <td style={{ padding: "9px 0", fontSize: 12, fontWeight: 500, color: "#999" }}>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div style={{ marginTop: 28 }}>
        <button aria-label={"Delete provider " + provider.displayName} onClick={() => { if (window.confirm("Delete " + provider.displayName + "?")) onDelete(provider.id); }} style={{ background: "#1C0808", color: "#FF453A", border: "1px solid #2A1010", borderRadius: 3, padding: "8px 16px", fontSize: 13, cursor: "pointer" }}>
          Delete provider
        </button>
      </div>
    </div>
  );
}
