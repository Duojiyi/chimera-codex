// Step 3.2/3.3 RED — Provider list state machine and switch logic.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  type ProviderListState,
  createInitialState,
  addProvider,
  switchProvider,
  deleteProvider,
  setHealth,
  selectActive,
  hydrateProviderView,
  type ProviderEntry,
} from "./providerState.ts";

function makeEntry(overrides: Partial<ProviderEntry> = {}): ProviderEntry {
  return {
    id: crypto.randomUUID(),
    displayName: "TestProvider",
    kind: "custom",
    baseUrl: "https://api.example.com/v1",
    protocol: "responses",
    secretRef: "keychain://chimera/test",
    selectedModel: null,
    health: "unknown",
    sortOrder: 0,
    ...overrides,
  };
}

// ── Initial state ────────────────────────────────────────────────────────────

describe("createInitialState", () => {
  it("starts with empty providers list", () => {
    const state = createInitialState();
    assert.equal(state.providers.length, 0);
    assert.equal(state.activeId, null);
  });

  it("official mode is default when no providers exist", () => {
    const state = createInitialState();
    assert.equal(state.officialMode, true);
  });
});

// ── addProvider ───────────────────────────────────────────────────────────────

describe("addProvider", () => {
  it("adds provider and sets it as active", () => {
    const state = addProvider(createInitialState(), makeEntry({ displayName: "MyAPI" }));
    assert.equal(state.providers.length, 1);
    assert.equal(state.providers[0].displayName, "MyAPI");
    assert.equal(state.activeId, state.providers[0].id);
  });

  it("adding ChimeraHub makes it active", () => {
    const hub = makeEntry({ kind: "chimera_hub", displayName: "ChimeraHub" });
    const state = addProvider(createInitialState(), hub);
    assert.equal(state.activeId, hub.id);
    assert.equal(state.officialMode, false);
  });

  it("adding second provider does not change active unless explicitly switched", () => {
    let state = createInitialState();
    const p1 = makeEntry({ displayName: "First" });
    const p2 = makeEntry({ displayName: "Second" });
    state = addProvider(state, p1);
    const activeAfterFirst = state.activeId;
    state = addProvider(state, p2);
    // Active should still be the first one added
    assert.equal(state.activeId, activeAfterFirst);
  });
});

// ── switchProvider ────────────────────────────────────────────────────────────

describe("switchProvider", () => {
  it("switches active provider", () => {
    let state = createInitialState();
    const p1 = makeEntry({ displayName: "P1" });
    const p2 = makeEntry({ displayName: "P2" });
    state = addProvider(addProvider(state, p1), p2);
    state = switchProvider(state, p2.id);
    assert.equal(state.activeId, p2.id);
    assert.equal(state.officialMode, false);
  });

  it("switching to null restores official mode", () => {
    let state = addProvider(createInitialState(), makeEntry());
    state = switchProvider(state, null);
    assert.equal(state.activeId, null);
    assert.equal(state.officialMode, true);
  });

  it("switching to non-existent id is ignored", () => {
    const state = addProvider(createInitialState(), makeEntry());
    const originalActiveId = state.activeId;
    const newState = switchProvider(state, "non-existent-id");
    assert.equal(newState.activeId, originalActiveId);
  });
});

// ── deleteProvider ────────────────────────────────────────────────────────────

describe("deleteProvider", () => {
  it("removes provider from list", () => {
    const p = makeEntry();
    let state = addProvider(createInitialState(), p);
    state = deleteProvider(state, p.id);
    assert.equal(state.providers.length, 0);
  });

  it("deleting active provider resets to official mode", () => {
    const p = makeEntry();
    let state = addProvider(createInitialState(), p);
    assert.equal(state.activeId, p.id);
    state = deleteProvider(state, p.id);
    assert.equal(state.activeId, null);
    assert.equal(state.officialMode, true);
  });
});

// ── setHealth ────────────────────────────────────────────────────────────────

describe("setHealth", () => {
  it("updates only health of target provider", () => {
    const p1 = makeEntry({ displayName: "P1" });
    const p2 = makeEntry({ displayName: "P2" });
    let state = addProvider(addProvider(createInitialState(), p1), p2);
    state = setHealth(state, p1.id, "healthy");
    const updated = state.providers.find(p => p.id === p1.id)!;
    const other = state.providers.find(p => p.id === p2.id)!;
    assert.equal(updated.health, "healthy");
    assert.equal(other.health, "unknown"); // unchanged
  });
});

// ── selectActive ─────────────────────────────────────────────────────────────

describe("selectActive", () => {
  it("returns null when in official mode", () => {
    const state = createInitialState();
    assert.equal(selectActive(state), null);
  });

  it("returns the active provider entry", () => {
    const p = makeEntry({ displayName: "Active" });
    const state = addProvider(createInitialState(), p);
    const active = selectActive(state);
    assert.ok(active);
    assert.equal(active.displayName, "Active");
  });
});

describe("hydrateProviderView", () => {
  it("keeps an externally configured provider out of official mode", () => {
    const hydrated = hydrateProviderView([], {
      activeProviderId: null,
      officialMode: false,
      providerName: "ChimeraHub",
      providerUrl: "https://api.chimerahub.org/v1",
    });

    assert.equal(hydrated.state.officialMode, false);
    assert.equal(hydrated.state.activeId, null);
    assert.deepEqual(hydrated.observedProvider, {
      displayName: "ChimeraHub",
      baseUrl: "https://api.chimerahub.org/v1",
    });
  });

  it("selects a saved provider instead of creating an observed duplicate", () => {
    const saved = makeEntry({ id: "saved", displayName: "ChimeraHub" });
    const hydrated = hydrateProviderView([saved], {
      activeProviderId: saved.id,
      officialMode: false,
      providerName: saved.displayName,
      providerUrl: saved.baseUrl,
    });

    assert.equal(hydrated.state.activeId, saved.id);
    assert.equal(hydrated.state.officialMode, false);
    assert.equal(hydrated.observedProvider, null);
  });
});
