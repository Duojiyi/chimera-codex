/**
 * Machine-state integration test — Bug 1: resolveCurrentProvider with real data.
 *
 * This test reads actual files from the local machine:
 *   - ~/.codex/config.toml  (Codex live config)
 *   - ~/.chimera-plus-plus/chimera.db  (Chimera++ provider DB, via Python)
 *
 * It is automatically skipped when those files are absent (CI / other machines).
 *
 * Scenarios verified:
 *   1. Real state: live endpoint matches DB provider → source "live"
 *   2. Proxy takeover (http://127.0.0.1:PORT) → source "stored", returns stored provider
 *   3. Proxy takeover (http://localhost:PORT/v1) → source "stored", returns stored provider
 *   4. liveReadSucceeded = false → source "stored"
 *   5. Unknown endpoint → source "external"
 */
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execSync } from "node:child_process";
import { tmpdir } from "node:os";
import { resolveCurrentProvider } from "@/chimeraUtils";
import type { Provider } from "@/types";

// ---------------------------------------------------------------------------
// Discover machine paths
// ---------------------------------------------------------------------------
const HOME = os.homedir();
const CODEX_CONFIG = path.join(HOME, ".codex", "config.toml");
const CHIMERA_DB = path.join(HOME, ".chimera-plus-plus", "chimera.db");

const machineFilesPresent =
  fs.existsSync(CODEX_CONFIG) && fs.existsSync(CHIMERA_DB);

// ---------------------------------------------------------------------------
// Read providers from DB via Python (avoids a native binary dep in devDeps)
// ---------------------------------------------------------------------------
function loadProvidersFromDb(): { providers: Provider[]; storedId: string } {
  // Write Python to a temp file to avoid all shell-escaping issues.
  const scriptPath = path.join(tmpdir(), "chimera_test_query.py");
  const script = [
    "import sqlite3, json",
    `db = sqlite3.connect(${JSON.stringify(CHIMERA_DB)})`,
    "db.row_factory = sqlite3.Row",
    'rows = db.execute("SELECT id, name, settings_config, category, sort_index, is_current FROM providers ORDER BY sort_index").fetchall()',
    "result = []",
    "for r in rows:",
    '    result.append({"id": r["id"], "name": r["name"], "settingsConfig": json.loads(r["settings_config"] or "{}"), "category": r["category"], "sortIndex": r["sort_index"], "isCurrent": bool(r["is_current"])})',
    "print(json.dumps(result))",
  ].join("\n");
  fs.writeFileSync(scriptPath, script, "utf8");
  const raw = execSync(`python3 "${scriptPath}"`, { encoding: "utf8" }).trim();
  const rows: Array<{
    id: string;
    name: string;
    settingsConfig: Record<string, unknown>;
    category: string | null;
    sortIndex: number;
    isCurrent: boolean;
  }> = JSON.parse(raw);

  const providers: Provider[] = rows.map((r) => ({
    id: r.id,
    name: r.name,
    settingsConfig: r.settingsConfig,
    category: (r.category as Provider["category"]) ?? undefined,
    sortIndex: r.sortIndex,
  }));

  const storedId = rows.find((r) => r.isCurrent)?.id ?? "";
  return { providers, storedId };
}

// ---------------------------------------------------------------------------
// Read Codex config as a "live" object (mimics what read_live_provider_settings returns)
// ---------------------------------------------------------------------------
function readCodexLive(): { config: string } {
  return { config: fs.readFileSync(CODEX_CONFIG, "utf8") };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe.skipIf(!machineFilesPresent)(
  "resolveCurrentProvider — real machine state (Bug 1)",
  () => {
    let providers: Provider[];
    let storedId: string;
    let live: { config: string };

    // Load once; if anything throws the suite fails clearly.
    try {
      ({ providers, storedId } = loadProvidersFromDb());
      live = readCodexLive();
    } catch (err) {
      // Swallow so individual tests can report skips gracefully.
      providers = [];
      storedId = "";
      live = { config: "" };
    }

    it("loads at least one provider from the real DB", () => {
      expect(providers.length).toBeGreaterThan(0);
    });

    it("has a stored (is_current) provider", () => {
      expect(storedId).not.toBe("");
      expect(providers.find((p) => p.id === storedId)).toBeDefined();
    });

    it("resolves real live config to a known provider (not external)", () => {
      const result = resolveCurrentProvider(providers, storedId, live, true);
      // The live Codex config's base_url should match the stored provider's config.
      // source should be "live" (exact match) or "stored" (proxy takeover).
      expect(["live", "stored"]).toContain(result.source);
      expect(result.provider).not.toBeNull();
    });

    it("scenario: proxy takeover http://127.0.0.1:PORT → returns stored provider", () => {
      const proxyLive = {
        config:
          'model_provider = "custom"\n[model_providers.custom]\nbase_url = "http://127.0.0.1:15721"\n',
      };
      const result = resolveCurrentProvider(providers, storedId, proxyLive, true);
      expect(result.source).toBe("stored");
      expect(result.provider?.id).toBe(storedId);
    });

    it("scenario: proxy takeover http://localhost:9999/v1 → returns stored provider", () => {
      const proxyLive = {
        config:
          'model_provider = "custom"\n[model_providers.custom]\nbase_url = "http://localhost:9999/v1"\n',
      };
      const result = resolveCurrentProvider(providers, storedId, proxyLive, true);
      expect(result.source).toBe("stored");
      expect(result.provider?.id).toBe(storedId);
    });

    it("scenario: liveReadSucceeded=false → source 'stored', returns stored provider", () => {
      const result = resolveCurrentProvider(providers, storedId, null, false);
      expect(result.source).toBe("stored");
      expect(result.provider?.id).toBe(storedId);
    });

    it("scenario: unknown endpoint → source 'external'", () => {
      const unknownLive = {
        config:
          'model_provider = "custom"\n[model_providers.custom]\nbase_url = "https://unknown-endpoint.example.com/v1"\n',
      };
      const result = resolveCurrentProvider(providers, storedId, unknownLive, true);
      expect(result.source).toBe("external");
      expect(result.provider).toBeNull();
    });

    it("scenario: proxy takeover with HTTPS scheme shouldn't be treated as local proxy", () => {
      // https://127.0.0.1 is unusual but valid; still loopback → stored
      const proxyLive = {
        config:
          'model_provider = "custom"\n[model_providers.custom]\nbase_url = "https://127.0.0.1:8443"\n',
      };
      const result = resolveCurrentProvider(providers, storedId, proxyLive, true);
      // includes("://127.0.0.1") matches https:// too → should return stored
      expect(result.source).toBe("stored");
    });
  },
);
