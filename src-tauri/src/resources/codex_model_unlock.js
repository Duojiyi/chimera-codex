(() => {
  "use strict";

  // This is intentionally a small, self-contained renderer patch. It does not
  // touch authentication state, credentials, or the app-server transport. Its
  // only job is to make the model catalog that Chimera++ already wrote visible
  // to Codex Desktop's model-picker data paths.
  const incoming = __CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__;
  const config = incoming && typeof incoming === "object" ? incoming : {};
  const modelEntries = Array.isArray(config.models) ? config.models : [];
  const modelNames = Array.from(new Set(
    modelEntries
      .map((entry) => typeof entry === "string" ? entry : entry?.model)
      .filter((name) => typeof name === "string" && name.trim())
      .map((name) => name.trim()),
  ));
  const defaultModel = typeof config.defaultModel === "string" && config.defaultModel.trim()
    ? config.defaultModel.trim()
    : modelNames[0] || "";
  const metadataByName = new Map(
    modelEntries
      .filter((entry) => entry && typeof entry === "object" && typeof entry.model === "string")
      .map((entry) => [entry.model.trim(), entry]),
  );

  globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__ = {
    ...config,
    models: modelEntries,
    modelNames,
    defaultModel,
  };

  const existingStatus = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__;
  if (globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_INSTALLED__) {
    const status = existingStatus && typeof existingStatus === "object" ? existingStatus : {};
    status.installed = true;
    status.modelCount = modelNames.length;
    globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
    return status;
  }

  globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_INSTALLED__ = true;
  globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = {
    installed: true,
    documentId: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    modelCount: modelNames.length,
    patched: 0,
    requestsSeen: 0,
    responsesSeen: 0,
    responsesPatched: 0,
    catalogVerified: false,
    lastModelListRequestId: null,
  };

  const names = () => {
    const current = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__;
    return Array.isArray(current?.modelNames) ? current.modelNames : modelNames;
  };
  const entryFor = (name) => {
    const current = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__;
    const currentEntries = Array.isArray(current?.models) ? current.models : modelEntries;
    return currentEntries.find((entry) => {
      const value = typeof entry === "string" ? entry : entry?.model;
      return value === name;
    }) || metadataByName.get(name) || {};
  };
  const modelKey = (value) => {
    if (!value || typeof value !== "object") return "";
    for (const key of ["model", "slug", "id", "name"]) {
      if (typeof value[key] === "string" && value[key].trim()) return value[key].trim();
    }
    return "";
  };
  const modelDescriptor = (name) => {
    const metadata = entryFor(name) || {};
    const displayName = typeof metadata.displayName === "string" && metadata.displayName.trim()
      ? metadata.displayName.trim()
      : name;
    const supportedReasoningEfforts = Array.isArray(metadata.supportedReasoningEfforts)
      ? metadata.supportedReasoningEfforts
      : ["low", "medium", "high", "xhigh"].map((reasoningEffort) => ({
          reasoningEffort,
          description: `${reasoningEffort} effort`,
        }));
    return {
      model: name,
      id: name,
      slug: name,
      name,
      displayName,
      description: typeof metadata.description === "string" && metadata.description.trim()
        ? metadata.description.trim()
        : "Third-party model",
      hidden: false,
      isDefault: name === defaultModel,
      defaultReasoningEffort: metadata.defaultReasoningEffort || "medium",
      supportedReasoningEfforts,
    };
  };
  const isStringArray = (value) => Array.isArray(value)
    && value.every((entry) => typeof entry === "string");
  const isModelObjectArray = (value) => Array.isArray(value)
    && value.length > 0
    && value.every((entry) => entry && typeof entry === "object"
      && typeof entry.model === "string" && entry.model.trim());
  const appendNames = (value) => {
    let changed = false;
    for (const name of names()) {
      if (!value.includes(name)) {
        value.push(name);
        changed = true;
      }
    }
    return changed;
  };
  const patchModelArray = (value, allowEmpty = false) => {
    if (!Array.isArray(value)) return false;
    const changedNames = names();
    if (!changedNames.length) return false;
    if (!(isModelObjectArray(value) || (allowEmpty && value.length === 0))) return false;

    let changed = false;
    const existing = new Set(value.map(modelKey).filter(Boolean));
    for (const item of value) {
      const key = modelKey(item);
      if (!key || !changedNames.includes(key)) continue;
      if (item.hidden !== false) {
        item.hidden = false;
        changed = true;
      }
      const descriptor = modelDescriptor(key);
      for (const field of ["displayName", "description", "defaultReasoningEffort"]) {
        if (descriptor[field] && item[field] !== descriptor[field]) {
          item[field] = descriptor[field];
          changed = true;
        }
      }
      if (Array.isArray(descriptor.supportedReasoningEfforts)
          && JSON.stringify(item.supportedReasoningEfforts || [])
             !== JSON.stringify(descriptor.supportedReasoningEfforts)) {
        item.supportedReasoningEfforts = descriptor.supportedReasoningEfforts;
        changed = true;
      }
    }
    for (const name of changedNames) {
      if (!existing.has(name)) {
        value.push(modelDescriptor(name));
        changed = true;
      }
    }
    return changed;
  };
  const catalogContainsConfiguredModels = (value) => {
    const found = new Set();
    const visit = (candidate, depth = 0) => {
      if (candidate == null || depth > 5) return;
      if (Array.isArray(candidate)) {
        for (const entry of candidate) {
          if (typeof entry === "string" && entry.trim()) found.add(entry.trim());
          else {
            const key = modelKey(entry);
            if (key) found.add(key);
            if (entry && typeof entry === "object") visit(entry, depth + 1);
          }
        }
        return;
      }
      if (typeof candidate !== "object") return;
      for (const key of ["models", "data", "result", "pages", "message", "response", "payload"]) {
        if (key in candidate) visit(candidate[key], depth + 1);
      }
    };
    visit(value);
    const configured = names();
    return configured.length > 0 && configured.every((name) => found.has(name));
  };

  const patchModelNameArray = (value, allowEmpty = false) => {
    if (!isStringArray(value) || (!allowEmpty && value.length === 0)) return false;
    return appendNames(value);
  };
  const patchSetOrArray = (value) => {
    const changedNames = names();
    if (value instanceof Set) {
      let changed = false;
      for (const name of changedNames) {
        if (!value.has(name)) {
          value.add(name);
          changed = true;
        }
      }
      return changed;
    }
    if (Array.isArray(value)) return appendNames(value);
    return false;
  };
  const removeHiddenNames = (value) => {
    if (!Array.isArray(value)) return false;
    const hidden = new Set(names());
    const next = value.filter((entry) => !hidden.has(entry));
    if (next.length === value.length) return false;
    value.splice(0, value.length, ...next);
    return true;
  };

  const modelPayloadLooksPatchable = (value) => {
    if (!value || typeof value !== "object") return false;
    if (Array.isArray(value)) return isModelObjectArray(value);
    const descriptorArrays = [
      value.models,
      value.data,
      value.result,
      value.pages?.[0]?.data,
      value.result?.data,
      value.result?.models,
      value.message?.result?.data,
      value.message?.result?.models,
    ];
    if (descriptorArrays.some(isModelObjectArray)) return true;
    const hasModelSignal = [
      "defaultModel", "default_model", "availableModels", "available_models",
      "hiddenModels", "hidden_models", "modelMetadata", "model_metadata",
    ].some((key) => key in value);
    return hasModelSignal && Array.isArray(value.models)
      && value.models.every((entry) => typeof entry === "string");
  };

  const patchContainer = (value, allowEmpty = false, depth = 0) => {
    if (!value || typeof value !== "object" || depth > 4) return false;
    if (Array.isArray(value)) return patchModelArray(value, allowEmpty);
    let changed = false;
    const hasModelSignal = [
      "defaultModel", "default_model", "availableModels", "available_models",
      "hiddenModels", "hidden_models", "modelMetadata", "model_metadata",
    ].some((key) => key in value);

    const allowEmptyModelArrays = allowEmpty || hasModelSignal;
    if (patchModelArray(value.models, allowEmptyModelArrays)) changed = true;
    if (hasModelSignal && patchModelNameArray(value.models, true)) changed = true;
    if (patchModelArray(value.data, allowEmpty)) changed = true;
    if (patchModelArray(value.result, allowEmpty)) changed = true;
    if (patchModelArray(value.result?.data, allowEmpty)) changed = true;
    if (patchModelArray(value.result?.models, allowEmpty)) changed = true;
    if (patchModelArray(value.pages?.[0]?.data, allowEmpty)) changed = true;
    if (patchModelArray(value.message?.result?.data, allowEmpty)) changed = true;
    if (patchModelArray(value.message?.result?.models, allowEmpty)) changed = true;

    if ("availableModels" in value && patchSetOrArray(value.availableModels)) changed = true;
    if ("available_models" in value && patchSetOrArray(value.available_models)) changed = true;
    if ("hiddenModels" in value && removeHiddenNames(value.hiddenModels)) changed = true;
    if ("hidden_models" in value && removeHiddenNames(value.hidden_models)) changed = true;

    const currentNames = names();
    if (hasModelSignal && value.defaultModel == null && currentNames.length) {
      value.defaultModel = modelDescriptor(defaultModel || currentNames[0]);
      changed = true;
    }
    if (hasModelSignal
        && ("default_model" in value || "available_models" in value)
        && value.default_model == null && currentNames.length) {
      value.default_model = defaultModel || currentNames[0];
      changed = true;
    }
    if (hasModelSignal && typeof value.model === "string"
        && !value.model.trim() && currentNames.length) {
      value.model = defaultModel || currentNames[0];
      changed = true;
    }

    // Catch only nested envelopes that independently look like model payloads.
    for (const key of ["result", "message", "response", "payload"]) {
      if (modelPayloadLooksPatchable(value[key])
          && patchContainer(value[key], allowEmpty, depth + 1)) changed = true;
    }
    if (changed) {
      const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
      status.patched = (status.patched || 0) + 1;
      globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
    }
    return changed;
  };

  const patchParsedJson = (payload) => {
    if (!modelPayloadLooksPatchable(payload)) return payload;
    try {
      patchContainer(payload, false);
    } catch {
      // Treat response patching as best effort.
    }
    return payload;
  };

  // Codex Desktop 2026.8+ connects the renderer to its app host through
  // Cap'n Web over MessagePort. The wire payload is a JSON string, so the
  // older WebSocket/window-message hooks never see model/list traffic.
  const nativeJsonParse = typeof JSON?.parse === "function" ? JSON.parse.bind(JSON) : null;
  const nativeJsonStringify = typeof JSON?.stringify === "function"
    ? JSON.stringify.bind(JSON)
    : null;
  const messagePortStates = new WeakMap();
  const messageListenerWrappers = new WeakMap();
  const onMessageListeners = new WeakMap();

  const portState = (port) => {
    let state = messagePortStates.get(port);
    if (!state) {
      state = { nextImportId: 1, modelListRequestIds: new Set() };
      messagePortStates.set(port, state);
    }
    return state;
  };
  const containsExactString = (value, expected, depth = 0) => {
    if (depth > 16) return false;
    if (value === expected) return true;
    if (!value || typeof value !== "object") return false;
    if (Array.isArray(value)) {
      return value.some((entry) => containsExactString(entry, expected, depth + 1));
    }
    return Object.values(value)
      .some((entry) => containsExactString(entry, expected, depth + 1));
  };
  const enableHiddenModelsInWireRequest = (value, depth = 0) => {
    if (!value || typeof value !== "object" || depth > 16) return;
    if (Array.isArray(value)) {
      for (const entry of value) enableHiddenModelsInWireRequest(entry, depth + 1);
      return;
    }
    if (value.method === "model/list") {
      value.params = { ...(value.params || {}), includeHidden: true };
    }
    if ("includeHidden" in value || "cursor" in value || "limit" in value) {
      value.includeHidden = true;
    }
    for (const entry of Object.values(value)) {
      enableHiddenModelsInWireRequest(entry, depth + 1);
    }
  };
  const encodeCapnWebValue = (value, depth = 0) => {
    if (depth > 32) return value;
    if (Array.isArray(value)) {
      return [value.map((entry) => encodeCapnWebValue(entry, depth + 1))];
    }
    if (value && typeof value === "object") {
      const encoded = {};
      for (const [key, entry] of Object.entries(value)) {
        encoded[key] = encodeCapnWebValue(entry, depth + 1);
      }
      return encoded;
    }
    return value;
  };
  const capnWebArrayItems = (value) => Array.isArray(value)
    && value.length === 1
    && Array.isArray(value[0])
    ? value[0]
    : null;
  const patchCapnWebModelArray = (value) => {
    const items = capnWebArrayItems(value);
    const currentNames = names();
    if (!items || !currentNames.length) return false;

    let changed = false;
    const existing = new Set(items.map(modelKey).filter(Boolean));
    for (const item of items) {
      const key = modelKey(item);
      if (!key || !currentNames.includes(key)) continue;
      const descriptor = modelDescriptor(key);
      if (item.hidden !== false) {
        item.hidden = false;
        changed = true;
      }
      for (const field of [
        "displayName", "description", "defaultReasoningEffort",
        "supportedReasoningEfforts",
      ]) {
        const encoded = encodeCapnWebValue(descriptor[field]);
        if (nativeJsonStringify?.(item[field]) !== nativeJsonStringify?.(encoded)) {
          item[field] = encoded;
          changed = true;
        }
      }
    }
    for (const name of currentNames) {
      if (!existing.has(name)) {
        items.push(encodeCapnWebValue(modelDescriptor(name)));
        changed = true;
      }
    }
    return changed;
  };
  const patchCapnWebModelListResult = (value, depth = 0) => {
    if (!value || typeof value !== "object" || depth > 8) return false;
    if (capnWebArrayItems(value)) return patchCapnWebModelArray(value);
    if (Array.isArray(value)) return false;

    let changed = false;
    for (const key of ["data", "models"]) {
      if (key in value && patchCapnWebModelArray(value[key])) changed = true;
    }
    for (const key of ["result", "message", "response", "payload"]) {
      if (key in value && patchCapnWebModelListResult(value[key], depth + 1)) changed = true;
    }
    return changed;
  };
  const capnWebCatalogContainsConfiguredModels = (value) => {
    const found = new Set();
    const visit = (candidate, depth = 0) => {
      if (depth > 16 || candidate == null) return;
      if (typeof candidate === "string") {
        found.add(candidate);
        return;
      }
      if (typeof candidate !== "object") return;
      for (const entry of Array.isArray(candidate)
        ? candidate
        : Object.values(candidate)) visit(entry, depth + 1);
    };
    visit(value);
    const currentNames = names();
    return currentNames.length > 0 && currentNames.every((name) => found.has(name));
  };
  const rememberModelListRequest = (state, requestId) => {
    const id = String(requestId);
    state.modelListRequestIds.add(id);
    if (state.modelListRequestIds.size > 64) {
      state.modelListRequestIds.delete(state.modelListRequestIds.values().next().value);
    }
    const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
    status.requestsSeen = (status.requestsSeen || 0) + 1;
    status.lastModelListRequestId = id;
    globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
    globalThis.setTimeout?.(() => state.modelListRequestIds.delete(id), 30_000);
  };
  const patchMessagePortOutgoing = (port, data) => {
    if (typeof data !== "string" || !nativeJsonParse || !nativeJsonStringify) return data;
    try {
      const wire = nativeJsonParse(data);
      if (!Array.isArray(wire)) return data;
      const state = portState(port);
      if (wire[0] === "push" || wire[0] === "stream" || wire[0] === "pipe") {
        const requestId = state.nextImportId;
        state.nextImportId += 1;
        if ((wire[0] === "push" || wire[0] === "stream")
            && containsExactString(wire[1], "model/list")) {
          enableHiddenModelsInWireRequest(wire[1]);
          rememberModelListRequest(state, requestId);
          return nativeJsonStringify(wire);
        }
      }
    } catch {
      // Ignore non-Cap'n-Web MessagePort traffic.
    }
    return data;
  };
  const patchMessagePortIncoming = (port, data) => {
    if (typeof data !== "string" || !nativeJsonParse || !nativeJsonStringify) return data;
    const state = messagePortStates.get(port);
    if (!state || state.modelListRequestIds.size === 0) return data;
    try {
      const wire = nativeJsonParse(data);
      if (!Array.isArray(wire) || (wire[0] !== "resolve" && wire[0] !== "reject")) {
        return data;
      }
      const requestId = wire[1] == null ? "" : String(wire[1]);
      if (!state.modelListRequestIds.has(requestId)) return data;
      state.modelListRequestIds.delete(requestId);

      const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
      status.responsesSeen = (status.responsesSeen || 0) + 1;
      if (wire[0] === "resolve") {
        const changed = patchCapnWebModelListResult(wire[2]);
        if (changed) {
          status.patched = (status.patched || 0) + 1;
          status.responsesPatched = (status.responsesPatched || 0) + 1;
        }
        status.catalogVerified = capnWebCatalogContainsConfiguredModels(wire[2]);
        globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
        return changed ? nativeJsonStringify(wire) : data;
      }
      status.catalogVerified = false;
      globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
    } catch {
      // Keep the original event if the protocol changes.
    }
    return data;
  };
  const messageEventWithData = (event, data) => {
    if (!event || data === event.data) return event;
    if (typeof MessageEvent !== "undefined") {
      try {
        return new MessageEvent(event.type || "message", {
          data,
          origin: event.origin || "",
          lastEventId: event.lastEventId || "",
          source: event.source || null,
          ports: event.ports ? Array.from(event.ports) : [],
        });
      } catch {
        // Fall through to a transparent proxy for non-browser test contexts.
      }
    }
    try {
      return new Proxy(event, {
        get(target, property) {
          if (property === "data") return data;
          const current = Reflect.get(target, property, target);
          return typeof current === "function" ? current.bind(target) : current;
        },
      });
    } catch {
      return { data };
    }
  };
  const wrapPortMessageListener = (port, listener) => {
    if (typeof listener !== "function"
        && (!listener || typeof listener.handleEvent !== "function")) return listener;
    let byPort = messageListenerWrappers.get(listener);
    if (!byPort) {
      byPort = new WeakMap();
      messageListenerWrappers.set(listener, byPort);
    }
    let wrapped = byPort.get(port);
    if (!wrapped) {
      wrapped = function chimeraCodexModelUnlockMessagePortListener(event) {
        const nextEvent = messageEventWithData(
          event,
          patchMessagePortIncoming(port, event?.data),
        );
        if (typeof listener === "function") return listener.call(this, nextEvent);
        return listener.handleEvent.call(listener, nextEvent);
      };
      byPort.set(port, wrapped);
    }
    return wrapped;
  };

  if (typeof MessagePort !== "undefined" && MessagePort.prototype) {
    const prototype = MessagePort.prototype;
    const originalPostMessage = prototype.postMessage;
    const originalAddEventListener = prototype.addEventListener;
    const originalRemoveEventListener = prototype.removeEventListener;
    if (typeof originalPostMessage === "function") {
      prototype.postMessage = function chimeraCodexModelUnlockMessagePortPostMessage(data, ...args) {
        return originalPostMessage.call(this, patchMessagePortOutgoing(this, data), ...args);
      };
    }
    if (typeof originalAddEventListener === "function") {
      prototype.addEventListener = function chimeraCodexModelUnlockMessagePortAddEventListener(
        type,
        listener,
        ...args
      ) {
        return originalAddEventListener.call(
          this,
          type,
          type === "message" ? wrapPortMessageListener(this, listener) : listener,
          ...args,
        );
      };
    }
    if (typeof originalRemoveEventListener === "function") {
      prototype.removeEventListener = function chimeraCodexModelUnlockMessagePortRemoveEventListener(
        type,
        listener,
        ...args
      ) {
        const canWrap = typeof listener === "function"
          || (listener && typeof listener === "object");
        const wrapped = type === "message" && canWrap
          ? messageListenerWrappers.get(listener)?.get(this) || listener
          : listener;
        return originalRemoveEventListener.call(this, type, wrapped, ...args);
      };
    }

    const onMessageDescriptor = Object.getOwnPropertyDescriptor(prototype, "onmessage");
    if (onMessageDescriptor?.get && onMessageDescriptor?.set && onMessageDescriptor.configurable) {
      Object.defineProperty(prototype, "onmessage", {
        configurable: true,
        enumerable: onMessageDescriptor.enumerable,
        get() {
          return onMessageListeners.get(this)?.listener
            ?? onMessageDescriptor.get.call(this);
        },
        set(listener) {
          if (listener == null) {
            onMessageListeners.delete(this);
            return onMessageDescriptor.set.call(this, listener);
          }
          const wrapped = wrapPortMessageListener(this, listener);
          onMessageListeners.set(this, { listener, wrapped });
          return onMessageDescriptor.set.call(this, wrapped);
        },
      });
    }
    const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
    status.messagePortPatched = true;
    globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
  }

  if (typeof JSON?.parse === "function") {
    const originalJsonParse = JSON.parse;
    JSON.parse = function chimeraCodexModelUnlockJsonParse(...args) {
      return patchParsedJson(originalJsonParse.apply(this, args));
    };
  }
  if (typeof Response !== "undefined" && typeof Response.prototype?.json === "function") {
    const originalResponseJson = Response.prototype.json;
    Response.prototype.json = async function chimeraCodexModelUnlockResponseJson(...args) {
      return patchParsedJson(await originalResponseJson.apply(this, args));
    };
  }

  const patchStatsigModelConfig = (statsigConfig) => {
    const value = statsigConfig?.value;
    const currentNames = names();
    if (!value || typeof value !== "object" || !currentNames.length) return statsigConfig;
    const available = Array.isArray(value.available_models)
      ? Array.from(new Set([...value.available_models, ...currentNames]))
      : [...currentNames];
    const nextValue = {
      ...value,
      available_models: available,
      default_model: defaultModel || currentNames[0] || value.default_model,
    };
    try {
      statsigConfig.value = nextValue;
      return statsigConfig;
    } catch {
      return { ...statsigConfig, value: nextValue };
    }
  };
  const patchStatsig = () => {
    const root = globalThis.__STATSIG__;
    if (!root || typeof root !== "object") return;
    const clients = [
      root.firstInstance,
      typeof root.instance === "function" ? root.instance() : null,
      ...(root.instances && typeof root.instances === "object" ? Object.values(root.instances) : []),
    ];
    for (const client of clients) {
      if (!client || typeof client.getDynamicConfig !== "function") continue;
      if (!client.__chimeraModelUnlockPatched) {
        const originalGetDynamicConfig = client.getDynamicConfig.bind(client);
        client.getDynamicConfig = (...args) => {
          const result = originalGetDynamicConfig(...args);
          return String(args[0]) === "107580212" ? patchStatsigModelConfig(result) : result;
        };
        client.__chimeraModelUnlockPatched = true;
      }
      try {
        patchStatsigModelConfig(client.getDynamicConfig("107580212", { disableExposureLog: true }));
      } catch {
        // Statsig is optional; the network/app-server patches remain active.
      }
    }
  };

  const modelListRequestIds = new Set();
  globalThis.addEventListener?.("codex-message-from-view", (event) => {
    try {
      const detail = event?.detail;
      const request = detail?.request;
      if (detail?.type === "mcp-request" && request?.method === "model/list") {
        request.params = { ...(request.params || {}), includeHidden: true };
        const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
        status.requestsSeen = (status.requestsSeen || 0) + 1;
        if (request.id != null) {
          const requestId = String(request.id);
          status.lastModelListRequestId = requestId;
          modelListRequestIds.add(requestId);
          if (modelListRequestIds.size > 64) {
            modelListRequestIds.delete(modelListRequestIds.values().next().value);
          }
          globalThis.setTimeout?.(() => modelListRequestIds.delete(requestId), 30_000);
        }
        globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
      }
    } catch {
      // Keep the renderer usable if a future event shape changes.
    }
  }, true);
  globalThis.addEventListener?.("message", (event) => {
    try {
      const data = event?.data;
      const message = data?.message || data?.response;
      const id = message?.id == null ? "" : String(message.id);
      if (data?.type === "mcp-response" && modelListRequestIds.has(id)) {
        modelListRequestIds.delete(id);
        const status = globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ || {};
        status.responsesSeen = (status.responsesSeen || 0) + 1;
        if (patchContainer(message?.result, true)) {
          status.responsesPatched = (status.responsesPatched || 0) + 1;
        }
        status.catalogVerified = catalogContainsConfiguredModels(message?.result);
        globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__ = status;
      } else {
        patchParsedJson(data);
      }
    } catch {
      // Ignore unrelated postMessage payloads.
    }
  }, true);

  patchStatsig();
  patchContainer(globalThis.__CODEX_MODEL_CATALOG__);
  globalThis.setInterval?.(patchStatsig, 250);

  return globalThis.__CHIMERA_CODEX_MODEL_UNLOCK_STATUS__;
})()
