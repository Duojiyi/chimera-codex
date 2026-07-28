import { describe, expect, it } from "vitest";
import type { Provider } from "@/types";
import {
  formatDuration,
  loadOperationRecords,
  resolveCurrentProvider,
  saveOperationRecords,
} from "./chimeraUtils";

function provider(id: string, endpoint: string, model: string): Provider {
  return {
    id,
    name: id,
    settingsConfig: {
      auth: {},
      config: `model = "${model}"\nmodel_provider = "custom"\n[model_providers.custom]\nbase_url = "${endpoint}"`,
    },
  } as Provider;
}

describe("resolveCurrentProvider", () => {
  const providers = [
    provider("first", "https://one.example/v1", "claude-a"),
    provider("second", "https://two.example/v1/", "claude-b"),
  ];

  it("matches the live endpoint instead of blindly trusting the stored id", () => {
    const result = resolveCurrentProvider(
      providers,
      "first",
      {
        config:
          'model = "claude-b"\nmodel_provider = "custom"\n[model_providers.custom]\nbase_url = "https://two.example/v1"',
      },
      true,
    );
    expect(result.provider?.id).toBe("second");
    expect(result.source).toBe("live");
  });

  it("reports an external configuration when no provider matches", () => {
    const result = resolveCurrentProvider(
      providers,
      "first",
      {
        config:
          'model = "other"\nmodel_provider = "custom"\n[model_providers.custom]\nbase_url = "https://external.example/v1"',
      },
      true,
    );
    expect(result.provider).toBeNull();
    expect(result.source).toBe("external");
  });
});

describe("operation records", () => {
  it("drops malformed records and persists newest first", () => {
    let value = JSON.stringify([{ nope: true }]);
    const storage = {
      getItem: () => value,
      setItem: (_key: string, next: string) => {
        value = next;
      },
    };
    expect(loadOperationRecords(storage)).toEqual([]);
    saveOperationRecords(
      [
        {
          id: "a",
          timestamp: 1,
          provider: "A",
          action: "切换",
          result: "success",
        },
        {
          id: "b",
          timestamp: 2,
          provider: "B",
          action: "测试",
          result: "error",
        },
      ],
      storage,
    );
    expect(loadOperationRecords(storage).map((record) => record.id)).toEqual([
      "b",
      "a",
    ]);
  });
});

describe("formatDuration", () => {
  it("formats milliseconds and seconds", () => {
    expect(formatDuration(240)).toBe("240ms");
    expect(formatDuration(1250)).toBe("1.3s");
    expect(formatDuration()).toBe("-");
  });
});
