/**
 * Regression cover for `resolveCurrentProvider`, the function that decides
 * which saved line the home screen calls "current" and where that answer came
 * from (`live` / `stored` / `external` / `none`).
 *
 * This used to read the developer's own `~/.codex/config.toml` and query
 * `~/.chimera-plus-plus/chimera.db` by shelling out to `python3`. That made it
 * fail on any machine without a Python interpreter and skip itself entirely
 * wherever those two files are absent — which includes CI, so the scenarios
 * below were never actually verified anywhere. `resolveCurrentProvider` is a
 * pure function, so the real machine bought nothing that a fixture does not.
 *
 * The fixture reproduces the state that motivated the original bug report: two
 * saved lines, one of them marked current, and a live config whose endpoint is
 * the local proxy rather than any saved upstream.
 */
import { describe, expect, it } from "vitest";
import { resolveCurrentProvider } from "@/chimeraUtils";
import type { Provider } from "@/types";

function codexConfig(baseUrl: string, model: string): string {
  return [
    `model = "${model}"`,
    'model_provider = "custom"',
    "[model_providers.custom]",
    'name = "custom"',
    `base_url = "${baseUrl}"`,
    'wire_api = "responses"',
    "",
  ].join("\n");
}

function provider(
  id: string,
  name: string,
  baseUrl: string,
  model: string,
  sortIndex: number,
): Provider {
  return {
    id,
    name,
    category: "custom",
    sortIndex,
    settingsConfig: { config: codexConfig(baseUrl, model) },
  } as Provider;
}

const STORED_ID = "line-primary";
const PROVIDERS: Provider[] = [
  provider(
    STORED_ID,
    "主线路",
    "https://api.chimerahub.org/v1",
    "gpt-5.6-sol",
    0,
  ),
  provider(
    "line-backup",
    "备用线路",
    "https://api.deepseek.com/v1",
    "deepseek-chat",
    1,
  ),
];

const liveFor = (baseUrl: string, model = "gpt-5.6-sol") => ({
  config: codexConfig(baseUrl, model),
});

describe("resolveCurrentProvider", () => {
  it("has a stored provider in the fixture", () => {
    expect(PROVIDERS.length).toBeGreaterThan(0);
    expect(PROVIDERS.find((entry) => entry.id === STORED_ID)).toBeDefined();
  });

  it("reports 'none' when no lines are saved", () => {
    const result = resolveCurrentProvider([], STORED_ID, null, false);
    expect(result.source).toBe("none");
    expect(result.provider).toBeNull();
  });

  it("matches a live endpoint to its saved line as 'live'", () => {
    const result = resolveCurrentProvider(
      PROVIDERS,
      STORED_ID,
      liveFor("https://api.chimerahub.org/v1"),
      true,
    );
    expect(result.source).toBe("live");
    expect(result.provider?.id).toBe(STORED_ID);
  });

  it("resolves a live endpoint belonging to a non-current line", () => {
    const result = resolveCurrentProvider(
      PROVIDERS,
      STORED_ID,
      liveFor("https://api.deepseek.com/v1", "deepseek-chat"),
      true,
    );
    expect(result.source).toBe("live");
    expect(result.provider?.id).toBe("line-backup");
  });

  // Under proxy takeover the live endpoint is Chimera's own loopback listener,
  // which no saved line can ever match. Falling through to "external" would
  // have shown the user "配置由外部程序管理" for a line Chimera itself set.
  it.each([
    ["http://127.0.0.1:15721", "bare loopback IP"],
    ["http://localhost:9999/v1", "localhost with a path"],
    ["https://127.0.0.1:8443", "https loopback"],
  ])("treats %s as proxy takeover and returns the stored line", (baseUrl) => {
    const result = resolveCurrentProvider(
      PROVIDERS,
      STORED_ID,
      liveFor(baseUrl),
      true,
    );
    expect(result.source).toBe("stored");
    expect(result.provider?.id).toBe(STORED_ID);
  });

  it("falls back to the stored line when the live config could not be read", () => {
    const result = resolveCurrentProvider(PROVIDERS, STORED_ID, null, false);
    expect(result.source).toBe("stored");
    expect(result.provider?.id).toBe(STORED_ID);
  });

  it("reports 'external' when the live config could not be read and nothing is stored", () => {
    const result = resolveCurrentProvider(PROVIDERS, "", null, false);
    expect(result.source).toBe("external");
    expect(result.provider).toBeNull();
  });

  it("reports 'external' for an endpoint no saved line uses", () => {
    const result = resolveCurrentProvider(
      PROVIDERS,
      STORED_ID,
      liveFor("https://unknown-endpoint.example.com/v1"),
      true,
    );
    expect(result.source).toBe("external");
    expect(result.provider).toBeNull();
  });

  it("keeps the stored line when neither config names an endpoint and the models disagree", () => {
    // An endpointless live config cannot be attributed by address, and the
    // differing model rules out an exact match, so the stored selection is the
    // only honest answer — calling it "external" would disown our own line.
    const endpointless = [
      {
        ...PROVIDERS[0],
        settingsConfig: { config: 'model = "gpt-5.6-sol"\n' },
      },
    ] as Provider[];
    const result = resolveCurrentProvider(
      endpointless,
      STORED_ID,
      { config: 'model = "gpt-6-astra"\n' },
      true,
    );
    expect(result.source).toBe("stored");
    expect(result.provider?.id).toBe(STORED_ID);
  });
});
