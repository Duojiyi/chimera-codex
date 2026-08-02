import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

import { describe, expect, it } from "vitest";

type RendererListener = (event: Record<string, unknown>) => void;

describe("Codex renderer model unlock script", () => {
  it("fills an empty no-auth model/list response with configured third-party models", () => {
    const raw = readFileSync(
      resolve("src-tauri/src/resources/codex_model_unlock.js"),
      "utf8",
    );
    const source = raw.replace(
      "__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__",
      JSON.stringify({
        defaultModel: "claude-opus-4-6",
        models: [
          { model: "claude-opus-4-6", displayName: "Claude Opus 4.6" },
          { model: "claude-sonnet-4-5", displayName: "Claude Sonnet 4.5" },
        ],
      }),
    );
    const listeners = new Map<string, RendererListener[]>();
    const context = {
      addEventListener(type: string, listener: RendererListener) {
        listeners.set(type, [...(listeners.get(type) ?? []), listener]);
      },
      setTimeout() {
        return 1;
      },
      setInterval() {
        return 1;
      },
    };
    vm.createContext(context);
    vm.runInContext(source, context);

    const request = { method: "model/list", id: 7, params: {} };
    for (const listener of listeners.get("codex-message-from-view") ?? []) {
      listener({ detail: { type: "mcp-request", request } });
    }
    const response = {
      type: "mcp-response",
      message: { id: 7, result: { data: [] as Array<{ model: string }> } },
    };
    for (const listener of listeners.get("message") ?? []) {
      listener({ data: response });
    }

    expect(request.params).toEqual({ includeHidden: true });
    expect(response.message.result.data.map((entry) => entry.model)).toEqual([
      "claude-opus-4-6",
      "claude-sonnet-4-5",
    ]);
    expect(
      (context as Record<string, any>).__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__,
    ).toMatchObject({
      installed: true,
      modelCount: 2,
      requestsSeen: 1,
      responsesSeen: 1,
      responsesPatched: 1,
      catalogVerified: true,
      lastModelListRequestId: "7",
    });
  });
});
