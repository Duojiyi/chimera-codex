// Chimera++ 2.0 — Provider list state machine (pure TS, no React, no I/O).
// G12: 此模块只做状态转换，不读写文件，不调用 Tauri invoke。

export type ProviderKind = "chimera_hub" | "custom";
export type ProviderHealth = "unknown" | "healthy" | "auth_failed" | "incompatible" | "unreachable";
export type ProviderProtocol = "responses";

export interface ProviderEntry {
  id: string;
  displayName: string;
  kind: ProviderKind;
  baseUrl: string;
  protocol: ProviderProtocol;
  secretRef: string | null;
  selectedModel: string | null;
  health: ProviderHealth;
  sortOrder: number;
}

export interface ProviderListState {
  providers: ProviderEntry[];
  /** null = Official Codex login mode */
  activeId: string | null;
  /** true when no custom provider is selected */
  officialMode: boolean;
}

export interface ProviderDetection {
  activeProviderId: string | null;
  officialMode: boolean;
  providerName: string | null;
  providerUrl: string | null;
}

export interface ObservedProvider {
  displayName: string;
  baseUrl: string;
}

export interface HydratedProviderView {
  state: ProviderListState;
  observedProvider: ObservedProvider | null;
}

// ── State constructors ────────────────────────────────────────────────────────

export function createInitialState(): ProviderListState {
  return { providers: [], activeId: null, officialMode: true };
}

/** Merge saved providers with the endpoint Codex is actually using. */
export function hydrateProviderView(
  providers: ProviderEntry[],
  detection: ProviderDetection,
): HydratedProviderView {
  const activeId = detection.activeProviderId !== null
    && providers.some((provider) => provider.id === detection.activeProviderId)
    ? detection.activeProviderId
    : null;
  const observedProvider = !detection.officialMode && activeId === null
    ? {
        displayName: detection.providerName?.trim() || "Custom provider",
        baseUrl: detection.providerUrl?.trim() || "",
      }
    : null;
  return {
    state: {
      providers,
      activeId,
      officialMode: detection.officialMode,
    },
    observedProvider,
  };
}

// ── Pure state transitions ────────────────────────────────────────────────────

/**
 * Add a new provider. First provider added becomes active.
 */
export function addProvider(
  state: ProviderListState,
  entry: ProviderEntry,
): ProviderListState {
  const isFirst = state.providers.length === 0;
  return {
    ...state,
    providers: [...state.providers, entry],
    activeId: isFirst ? entry.id : state.activeId,
    officialMode: isFirst ? false : state.officialMode,
  };
}

/**
 * Switch to a different provider (or null = restore Official mode).
 * Silently ignores non-existent ids.
 */
export function switchProvider(
  state: ProviderListState,
  targetId: string | null,
): ProviderListState {
  if (targetId !== null && !state.providers.some(p => p.id === targetId)) {
    return state; // non-existent id → no change
  }
  return {
    ...state,
    activeId: targetId,
    officialMode: targetId === null,
  };
}

/**
 * Delete a provider. If deleted provider was active, revert to Official mode.
 */
export function deleteProvider(
  state: ProviderListState,
  id: string,
): ProviderListState {
  const remaining = state.providers.filter(p => p.id !== id);
  const wasActive = state.activeId === id;
  return {
    ...state,
    providers: remaining,
    activeId: wasActive ? null : state.activeId,
    officialMode: wasActive ? true : state.officialMode,
  };
}

/**
 * Update health status for a single provider.
 */
export function setHealth(
  state: ProviderListState,
  id: string,
  health: ProviderHealth,
): ProviderListState {
  return {
    ...state,
    providers: state.providers.map(p =>
      p.id === id ? { ...p, health } : p,
    ),
  };
}

// ── Selectors ─────────────────────────────────────────────────────────────────

/** Returns the currently active provider, or null if in Official mode. */
export function selectActive(state: ProviderListState): ProviderEntry | null {
  if (state.activeId === null) return null;
  return state.providers.find(p => p.id === state.activeId) ?? null;
}
