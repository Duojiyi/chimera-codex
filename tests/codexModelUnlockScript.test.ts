import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

import { describe, expect, it } from "vitest";

type RendererListener = (event: Record<string, any>) => void;

type TestContext = Record<string, any> & {
  __CHIMERA_CODEX_MODEL_UNLOCK_STATUS__?: Record<string, any>;
};

const config = {
  defaultModel: "claude-opus-4-6",
  models: [
    { model: "claude-opus-4-6", displayName: "Claude Opus 4.6" },
    { model: "claude-sonnet-4-5", displayName: "Claude Sonnet 4.5" },
  ],
};

function loadRenderer() {
  const raw = readFileSync(
    resolve("src-tauri/src/resources/codex_model_unlock.js"),
    "utf8",
  );
  const source = raw.replace(
    "__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__",
    JSON.stringify(config),
  );
  const listeners = new Map<string, RendererListener[]>();

  class FakeMessagePort {
    readonly sent: string[] = [];
    private readonly listeners = new Map<string, RendererListener[]>();
    private messageListener: RendererListener | null = null;

    postMessage(data: string) {
      this.sent.push(data);
    }

    addEventListener(type: string, listener: RendererListener) {
      this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
    }

    removeEventListener(type: string, listener: RendererListener) {
      this.listeners.set(
        type,
        (this.listeners.get(type) ?? []).filter((candidate) => candidate !== listener),
      );
    }

    emit(data: string) {
      const event = { type: "message", data };
      for (const listener of this.listeners.get("message") ?? []) {
        listener.call(this, event);
      }
    }

    get onmessage() {
      return this.messageListener;
    }

    set onmessage(listener: RendererListener | null) {
      this.messageListener = listener;
      if (listener) this.addEventListener("message", listener);
    }
  }

  const context: TestContext = {
    MessagePort: FakeMessagePort,
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
  return { context, listeners, FakeMessagePort };
}

describe("Codex renderer model unlock script", () => {
  it("fills an empty no-auth model/list response through the legacy view message path", () => {
    const { context, listeners } = loadRenderer();
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
    expect(context.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__).toMatchObject({
      installed: true,
      modelCount: 2,
      requestsSeen: 1,
      responsesSeen: 1,
      responsesPatched: 1,
      catalogVerified: true,
      lastModelListRequestId: "7",
    });
  });

  it("patches an empty Cap'n Web MessagePort model/list response", () => {
    const { context, FakeMessagePort } = loadRenderer();
    const port = new FakeMessagePort();
    const received: string[] = [];
    port.addEventListener("message", (event) => received.push(event.data));

    port.postMessage(JSON.stringify([
      "push",
      ["pipeline", 0, "sendRequest", "model/list", { cursor: null, limit: 100 }],
    ]));
    const sent = JSON.parse(port.sent[0]);
    expect(sent[1]).toContain("model/list");
    expect(sent[1]).toContainEqual(expect.objectContaining({ includeHidden: true }));

    port.emit(JSON.stringify(["resolve", 1, { data: [[]], nextCursor: null }]));
    const response = JSON.parse(received[0]);
    expect(response[2].data[0].map((entry: { model: string }) => entry.model)).toEqual([
      "claude-opus-4-6",
      "claude-sonnet-4-5",
    ]);
    expect(context.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__).toMatchObject({
      messagePortPatched: true,
      requestsSeen: 1,
      responsesSeen: 1,
      responsesPatched: 1,
      catalogVerified: true,
      lastModelListRequestId: "1",
    });
  });

  it("appends configured models to an existing Cap'n Web catalog and leaves unrelated responses alone", () => {
    const { context, FakeMessagePort } = loadRenderer();
    const port = new FakeMessagePort();
    const received: string[] = [];
    port.addEventListener("message", (event) => received.push(event.data));

    port.postMessage(JSON.stringify([
      "push",
      ["pipeline", 0, "sendRequest", "thread/list", { cursor: null, limit: 10 }],
    ]));
    port.emit(JSON.stringify(["resolve", 1, { data: [[]] }]));
    expect(JSON.parse(received[0])).toEqual(["resolve", 1, { data: [[]] }]);

    port.postMessage(JSON.stringify([
      "push",
      ["pipeline", 0, "sendRequest", "model/list", { cursor: null, limit: 100 }],
    ]));
    port.emit(JSON.stringify([
      "resolve",
      2,
      {
        data: [[{ model: "gpt-5", hidden: false }]],
        nextCursor: null,
      },
    ]));
    const response = JSON.parse(received[1]);
    expect(response[2].data[0].map((entry: { model: string }) => entry.model)).toEqual([
      "gpt-5",
      "claude-opus-4-6",
      "claude-sonnet-4-5",
    ]);
    expect(context.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__).toMatchObject({
      requestsSeen: 1,
      responsesSeen: 1,
      responsesPatched: 1,
      catalogVerified: true,
      lastModelListRequestId: "2",
    });
  });
});
